use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    io::{self, Write},
};

use num_rational::BigRational;
use num_traits::{Signed, Zero};
use ori_domain::{CreasePattern, EdgeId, EdgeKind, Paper, VertexId};
use ori_topology::{FoldAssignment, TopologySnapshot};
use sha2::{Digest, Sha256};

use crate::{
    FacePairOrderSnapshot, FacewiseConstraintKind, FacewiseProofSummary,
    FlatFoldabilityProofIncompleteReason, FlatFoldabilityResource, FoldedFaceOrientation,
    FoldedFaceSnapshot, GlobalFlatFoldabilityCheckpoint, GlobalFlatFoldabilityExecutionError,
    GlobalFlatFoldabilityImpossibleReason, GlobalFlatFoldabilityInternalError,
    GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityObserver, GlobalFlatFoldabilityOutcome,
    GlobalFlatFoldabilityPhase, GlobalFlatFoldabilityPossibleReason,
    GlobalFlatFoldabilityProvenance, GlobalFlatFoldabilityReport,
    GlobalFlatFoldabilityUnknownReason, GlobalFlatFoldabilityWorkCounts, LayerFace,
    LayerOrderDerivation, LayerOrderModelId, LayerOrderProvenance, LayerOrderSnapshot,
    OverlapCellKey, OverlapCellSnapshot, RequiredLayerOrderError, RequiredLayerOrderPair,
    UnsupportedFlatFoldabilityTopology, complete_progress,
    constraints::{
        CompleteAssignmentVerificationResult, ConstraintConflict, ConstraintSet,
        ConstraintSolverControl, ConstraintSolverEvent, ConstraintSolverResult, ConstraintView,
        TransitivityConstraintFamily, TransitivityConstraints, TupleConstraint, choose_three,
        choose_two, solve_constraints_with_memory, verify_complete_assignment_with_memory,
    },
    exact::{
        self, ExactBudget, ExactError, Point, Rational, Transform, add, apply, average3, cmp,
        compose, cross, div, midpoint, mul, point_from_binary64, rational_bytes, reflection_across,
        signed_double_area, sub,
    },
    unknown,
};

const CELL_KEY_DOMAIN: &[u8] = b"ORIGAMI2\0overlap-cell\0v1\0";
const CONTROL_POLL_RECORDS: usize = 1_024;
const SERIALIZATION_POLL_BYTES: usize = 64 * 1_024;
const FACE_INTERIOR_LEFT: u8 = 1;
const FACE_INTERIOR_RIGHT: u8 = 2;
const TACO_TACO_VALID_SOURCE_TUPLES: [&str; 16] = [
    "111112", "111121", "111222", "112111", "121112", "121222", "122111", "122212", "211121",
    "211222", "212111", "212221", "221222", "222111", "222212", "222221",
];

type FacewiseResult<T> = Result<T, FacewiseAbort>;

#[derive(Debug, Default, Clone, Copy)]
struct ExactStorage {
    embedding_bytes: usize,
    arrangement_bytes: usize,
    snapshot_bytes: usize,
    certificate_structure_bytes: usize,
    verification_bytes: usize,
    constraint_bytes: usize,
}

impl ExactStorage {
    fn total(&self) -> Option<usize> {
        self.embedding_bytes
            .checked_add(self.arrangement_bytes)?
            .checked_add(self.snapshot_bytes)?
            .checked_add(self.certificate_structure_bytes)?
            .checked_add(self.verification_bytes)?
            .checked_add(self.constraint_bytes)
    }
}

#[derive(Debug)]
enum FacewiseAbort {
    Unknown(GlobalFlatFoldabilityUnknownReason),
    Impossible(GlobalFlatFoldabilityImpossibleReason),
    RequiredLayerOrder(RequiredLayerOrderError),
    Execution(GlobalFlatFoldabilityExecutionError),
}

impl From<GlobalFlatFoldabilityExecutionError> for FacewiseAbort {
    fn from(value: GlobalFlatFoldabilityExecutionError) -> Self {
        Self::Execution(value)
    }
}

impl From<ExactError> for FacewiseAbort {
    fn from(value: ExactError) -> Self {
        match value {
            ExactError::NonFiniteBinary64 | ExactError::NegativeZero => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::InvalidBinary64Coordinate,
                })
            }
            ExactError::DegenerateDivision => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                })
            }
            ExactError::IntegerBitLimitReached {
                limit_bits,
                observed_bits,
            } => Self::Unknown(
                GlobalFlatFoldabilityUnknownReason::ExactNumberLimitReached {
                    limit_bits,
                    observed_bits,
                },
            ),
            ExactError::WorkLimitReached { limit, observed } => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::ExactOperations,
                    limit,
                    observed,
                })
            }
            ExactError::DeadlineReached { phase } => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::TimeLimitReached { phase })
            }
            ExactError::Cancelled => {
                Self::Execution(GlobalFlatFoldabilityExecutionError::Cancelled)
            }
            ExactError::InternalFailure => {
                Self::Execution(GlobalFlatFoldabilityExecutionError::Internal {
                    reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
                })
            }
        }
    }
}

struct Runtime<'a, O: GlobalFlatFoldabilityObserver + ?Sized> {
    observer: &'a mut O,
    limits: GlobalFlatFoldabilityLimits,
    work: GlobalFlatFoldabilityWorkCounts,
    phase: GlobalFlatFoldabilityPhase,
    exact_storage: ExactStorage,
}

impl<'a, O: GlobalFlatFoldabilityObserver + ?Sized> Runtime<'a, O> {
    fn new(
        observer: &'a mut O,
        limits: GlobalFlatFoldabilityLimits,
        work: GlobalFlatFoldabilityWorkCounts,
    ) -> Self {
        Self {
            observer,
            limits,
            work,
            phase: GlobalFlatFoldabilityPhase::ValidatingLocalConditions,
            exact_storage: ExactStorage::default(),
        }
    }

    fn advance(
        &mut self,
        phase: GlobalFlatFoldabilityPhase,
        total_work: Option<usize>,
    ) -> FacewiseResult<()> {
        if phase <= self.phase {
            return Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Internal {
                    reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
                },
            ));
        }
        self.phase = phase;
        self.progress();
        self.checkpoint(total_work)
    }

    fn checkpoint(&mut self, _total_work: Option<usize>) -> FacewiseResult<()> {
        match self.observer.checkpoint() {
            GlobalFlatFoldabilityCheckpoint::Continue => Ok(()),
            GlobalFlatFoldabilityCheckpoint::DeadlineReached => Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached { phase: self.phase },
            )),
            GlobalFlatFoldabilityCheckpoint::Cancelled => Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled,
            )),
        }
    }

    fn poll_control(&mut self, pending_records: &mut usize) -> FacewiseResult<()> {
        *pending_records = pending_records.saturating_add(1);
        if *pending_records >= CONTROL_POLL_RECORDS {
            *pending_records = 0;
            self.checkpoint(None)?;
        }
        Ok(())
    }

    fn progress(&mut self) {
        self.observer
            .on_progress(crate::GlobalFlatFoldabilityProgress {
                phase: self.phase,
                completed_work: self
                    .work
                    .total_records
                    .saturating_add(self.work.arrangement_segments)
                    .saturating_add(self.work.constraints)
                    .saturating_add(self.work.search_nodes),
                total_work: None,
                exact_operations: self.work.exact_operations,
                overlap_face_pairs: self.work.overlap_face_pairs,
                overlap_cells: self.work.overlap_cells,
                constraints: self.work.constraints,
                search_nodes: self.work.search_nodes,
            });
    }

    fn set_overlap_pairs(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_overlap_face_pairs {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::OverlapFacePairs,
                    limit: self.limits.max_overlap_face_pairs,
                    observed,
                },
            ));
        }
        self.work.overlap_face_pairs = observed;
        Ok(())
    }

    fn set_arrangement_segments(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_arrangement_segments {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                    resource: FlatFoldabilityResource::ArrangementSegments,
                    limit: self.limits.max_arrangement_segments,
                    observed,
                },
            ));
        }
        self.work.arrangement_segments = observed;
        Ok(())
    }

    fn set_overlap_cells(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_overlap_cells {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                    resource: FlatFoldabilityResource::OverlapCells,
                    limit: self.limits.max_overlap_cells,
                    observed,
                },
            ));
        }
        self.work.overlap_cells = observed;
        Ok(())
    }

    fn set_constraints(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_constraints {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                    limit: self.limits.max_constraints,
                    observed,
                },
            ));
        }
        self.work.constraints = observed;
        Ok(())
    }

    fn set_search_nodes(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_search_nodes {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::SearchNodes,
                    limit: self.limits.max_search_nodes,
                    observed,
                },
            ));
        }
        self.work.search_nodes = observed;
        Ok(())
    }

    fn set_certificate_bytes(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_certificate_bytes {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: self.limits.max_certificate_bytes,
                    observed,
                },
            ));
        }
        self.work.certificate_bytes = observed;
        Ok(())
    }

    fn exact_storage_limit_failure(&self, observed: usize) -> FacewiseAbort {
        FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
            resource: FlatFoldabilityResource::CertificateBytes,
            limit: self.limits.max_certificate_bytes,
            observed,
        })
    }

    fn ensure_storage_values(
        &self,
        storage: ExactStorage,
        transient_bytes: usize,
    ) -> FacewiseResult<usize> {
        let values = [
            storage.embedding_bytes,
            storage.arrangement_bytes,
            storage.snapshot_bytes,
            storage.certificate_structure_bytes,
            storage.verification_bytes,
            storage.constraint_bytes,
            transient_bytes,
        ];
        // `usize::MAX` is also the sentinel produced by the storage-size
        // helpers on arithmetic overflow. Reject it even when a caller
        // configures an unbounded-looking `usize::MAX` limit.
        if values.contains(&usize::MAX) {
            return Err(self.exact_storage_limit_failure(usize::MAX));
        }
        let observed = values.into_iter().try_fold(0_usize, |total, value| {
            total
                .checked_add(value)
                .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))
        })?;
        if observed > self.limits.max_certificate_bytes {
            return Err(self.exact_storage_limit_failure(observed));
        }
        Ok(observed)
    }

    fn set_embedding_exact_storage(&mut self, observed: usize) -> FacewiseResult<()> {
        let mut candidate = self.exact_storage;
        candidate.embedding_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.embedding_bytes = observed;
        Ok(())
    }

    fn add_embedding_exact_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .embedding_bytes
            .saturating_add(additional);
        let mut candidate = self.exact_storage;
        candidate.embedding_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.embedding_bytes = observed;
        Ok(())
    }

    fn set_arrangement_exact_storage(&mut self, observed: usize) -> FacewiseResult<()> {
        let mut candidate = self.exact_storage;
        candidate.arrangement_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.arrangement_bytes = observed;
        Ok(())
    }

    fn add_snapshot_exact_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self.exact_storage.snapshot_bytes.saturating_add(additional);
        let mut candidate = self.exact_storage;
        candidate.snapshot_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.snapshot_bytes = observed;
        Ok(())
    }

    fn ensure_transient_exact_storage(&self, additional: usize) -> FacewiseResult<()> {
        self.ensure_storage_values(self.exact_storage, additional)?;
        Ok(())
    }

    fn add_certificate_structure_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .certificate_structure_bytes
            .checked_add(additional)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        let mut candidate = self.exact_storage;
        candidate.certificate_structure_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.certificate_structure_bytes = observed;
        Ok(())
    }

    fn add_verification_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .verification_bytes
            .checked_add(additional)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        let mut candidate = self.exact_storage;
        candidate.verification_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.verification_bytes = observed;
        Ok(())
    }

    fn add_constraint_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .constraint_bytes
            .checked_add(additional)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        let mut candidate = self.exact_storage;
        candidate.constraint_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.constraint_bytes = observed;
        Ok(())
    }

    fn ensure_constraint_transient_storage(&self, additional: usize) -> FacewiseResult<()> {
        self.ensure_storage_values(self.exact_storage, additional)?;
        Ok(())
    }

    fn clear_constraint_storage(&mut self) {
        self.exact_storage.constraint_bytes = 0;
    }

    fn release_constraint_storage(&mut self, released: usize) -> FacewiseResult<()> {
        self.exact_storage.constraint_bytes = self
            .exact_storage
            .constraint_bytes
            .checked_sub(released)
            .ok_or_else(internal_abort)?;
        Ok(())
    }

    fn remaining_storage_bytes(&self) -> FacewiseResult<usize> {
        let used = self
            .exact_storage
            .total()
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        self.limits
            .max_certificate_bytes
            .checked_sub(used)
            .ok_or_else(|| self.exact_storage_limit_failure(used))
    }

    fn clear_verification_storage(&mut self) {
        self.exact_storage.verification_bytes = 0;
    }

    fn verification_storage_bytes(&self) -> usize {
        self.exact_storage.verification_bytes
    }

    fn restore_verification_storage(&mut self, retained: usize) {
        debug_assert!(retained <= self.exact_storage.verification_bytes);
        self.exact_storage.verification_bytes = retained;
    }

    fn allocation_bytes(&self, count: usize, element_size: usize) -> FacewiseResult<usize> {
        count
            .checked_mul(element_size)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))
    }

    fn constraint_solver_control(
        &mut self,
        event: ConstraintSolverEvent,
        search_nodes: usize,
    ) -> ConstraintSolverControl {
        if search_nodes <= self.limits.max_search_nodes {
            self.work.search_nodes = search_nodes;
        }
        let target_phase = match event {
            ConstraintSolverEvent::PropagationBatch => None,
            ConstraintSolverEvent::SearchNode => Some(GlobalFlatFoldabilityPhase::Searching),
            ConstraintSolverEvent::VerifyingConstraint => {
                Some(GlobalFlatFoldabilityPhase::VerifyingCertificate)
            }
        };
        if let Some(target_phase) = target_phase
            && target_phase > self.phase
        {
            self.phase = target_phase;
            self.progress();
        }
        if matches!(event, ConstraintSolverEvent::SearchNode) && search_nodes.is_multiple_of(1_024)
        {
            self.progress();
        }
        match self.observer.checkpoint() {
            GlobalFlatFoldabilityCheckpoint::Continue => ConstraintSolverControl::Continue,
            GlobalFlatFoldabilityCheckpoint::DeadlineReached => {
                ConstraintSolverControl::DeadlineReached
            }
            GlobalFlatFoldabilityCheckpoint::Cancelled => ConstraintSolverControl::Cancelled,
        }
    }
}

impl<O: GlobalFlatFoldabilityObserver + ?Sized> ExactBudget for Runtime<'_, O> {
    fn record_exact_operation(&mut self) -> Result<(), ExactError> {
        let observed =
            self.work
                .exact_operations
                .checked_add(1)
                .ok_or(ExactError::WorkLimitReached {
                    limit: self.limits.max_exact_operations,
                    observed: usize::MAX,
                })?;
        if observed > self.limits.max_exact_operations {
            return Err(ExactError::WorkLimitReached {
                limit: self.limits.max_exact_operations,
                observed,
            });
        }
        self.work.exact_operations = observed;
        if observed % 1_024 == 0 {
            self.progress();
            match self.observer.checkpoint() {
                GlobalFlatFoldabilityCheckpoint::Continue => {}
                GlobalFlatFoldabilityCheckpoint::DeadlineReached => {
                    return Err(ExactError::DeadlineReached { phase: self.phase });
                }
                GlobalFlatFoldabilityCheckpoint::Cancelled => {
                    return Err(ExactError::Cancelled);
                }
            }
        }
        Ok(())
    }

    fn record_exact_value(&mut self, value: &BigRational) -> Result<(), ExactError> {
        self.work.exact_values =
            self.work
                .exact_values
                .checked_add(1)
                .ok_or(ExactError::WorkLimitReached {
                    limit: self.limits.max_exact_operations,
                    observed: usize::MAX,
                })?;
        let observed_bits = exact::bit_len(value)?;
        if observed_bits > self.limits.max_exact_integer_bits {
            return Err(ExactError::IntegerBitLimitReached {
                limit_bits: self.limits.max_exact_integer_bits,
                observed_bits,
            });
        }
        Ok(())
    }
}

fn exact_storage_bytes_point(point: &Point) -> Result<usize, ExactError> {
    Ok(exact::rational_storage_bytes(&point.x)?
        .saturating_add(exact::rational_storage_bytes(&point.y)?))
}

fn exact_storage_bytes_points(points: &[Point]) -> Result<usize, ExactError> {
    points.iter().try_fold(0_usize, |total, point| {
        Ok(total.saturating_add(exact_storage_bytes_point(point)?))
    })
}

fn exact_storage_bytes_transform(transform: &Transform) -> Result<usize, ExactError> {
    [
        &transform.m00,
        &transform.m01,
        &transform.m10,
        &transform.m11,
        &transform.tx,
        &transform.ty,
    ]
    .into_iter()
    .try_fold(0_usize, |total, value| {
        Ok(total.saturating_add(exact::rational_storage_bytes(value)?))
    })
}

fn exact_storage_bytes_embedding(embedding: &FlatEmbedding) -> Result<usize, ExactError> {
    let mut total = 0_usize;
    for face in &embedding.faces {
        total = total
            .saturating_add(exact_storage_bytes_points(&face.source.source_polygon)?)
            .saturating_add(exact_storage_bytes_transform(&face.transform)?)
            .saturating_add(exact_storage_bytes_points(&face.polygon)?);
    }
    for hinge in &embedding.hinges {
        total = total
            .saturating_add(exact_storage_bytes_point(&hinge.first_point)?)
            .saturating_add(exact_storage_bytes_point(&hinge.second_point)?);
    }
    Ok(total)
}

#[derive(Clone)]
struct SourceEdge {
    start: VertexId,
    end: VertexId,
    kind: EdgeKind,
}

#[derive(Clone)]
struct SourceFace {
    layer: LayerFace,
    vertex_ids: Vec<VertexId>,
    source_polygon: Vec<Point>,
}

#[derive(Clone)]
struct FoldedFace {
    source: SourceFace,
    transform: Transform,
    front_up: bool,
    polygon: Vec<Point>,
}

#[derive(Clone)]
struct FoldedHinge {
    edge: EdgeId,
    first_face: usize,
    second_face: usize,
    assignment: FoldAssignment,
    first_point: Point,
    second_point: Point,
}

#[derive(Clone)]
struct FlatEmbedding {
    reference_face: usize,
    faces: Vec<FoldedFace>,
    hinges: Vec<FoldedHinge>,
    material_internal_edge_count: usize,
}

struct SolveSuccess {
    reason: GlobalFlatFoldabilityPossibleReason,
    layer_order: LayerOrderSnapshot,
}

pub(crate) struct FacewiseAnalysisInput<'a> {
    pub(crate) paper: &'a Paper,
    pub(crate) crease_pattern: &'a CreasePattern,
    pub(crate) topology: &'a TopologySnapshot,
    pub(crate) canonical_faces: &'a [LayerFace],
    pub(crate) provenance: GlobalFlatFoldabilityProvenance,
    pub(crate) work_counts: GlobalFlatFoldabilityWorkCounts,
    pub(crate) limits: GlobalFlatFoldabilityLimits,
}

pub(crate) fn analyze_facewise<O: GlobalFlatFoldabilityObserver + ?Sized>(
    input: FacewiseAnalysisInput<'_>,
    observer: &mut O,
) -> Result<GlobalFlatFoldabilityReport, GlobalFlatFoldabilityExecutionError> {
    let FacewiseAnalysisInput {
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        work_counts,
        limits,
    } = input;
    let mut runtime = Runtime::new(observer, limits, work_counts);
    let result = solve_facewise(
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        None,
        &mut runtime,
    );
    match result {
        Ok(success) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(GlobalFlatFoldabilityReport {
                provenance,
                work_counts: runtime.work,
                outcome: GlobalFlatFoldabilityOutcome::Possible {
                    reason: success.reason,
                    layer_order: Box::new(success.layer_order),
                },
                analysis_seal: super::GlobalFlatFoldabilityAnalysisSealV2,
            })
        }
        Err(FacewiseAbort::Unknown(reason)) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(unknown(provenance, runtime.work, reason))
        }
        Err(FacewiseAbort::Impossible(reason)) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(GlobalFlatFoldabilityReport {
                provenance,
                work_counts: runtime.work,
                outcome: GlobalFlatFoldabilityOutcome::Impossible { reason },
                analysis_seal: super::GlobalFlatFoldabilityAnalysisSealV2,
            })
        }
        Err(FacewiseAbort::RequiredLayerOrder(_)) => {
            Err(GlobalFlatFoldabilityExecutionError::Internal {
                reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
            })
        }
        Err(FacewiseAbort::Execution(error)) => Err(error),
    }
}

pub(crate) fn analyze_facewise_with_required_pair_orders<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    input: FacewiseAnalysisInput<'_>,
    required_pair_orders: &[RequiredLayerOrderPair],
    observer: &mut O,
) -> Result<LayerOrderSnapshot, RequiredLayerOrderError> {
    let FacewiseAnalysisInput {
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        work_counts,
        limits,
    } = input;
    let mut runtime = Runtime::new(observer, limits, work_counts);
    let required_pair_orders = (!required_pair_orders.is_empty()).then_some(required_pair_orders);
    match solve_facewise(
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        required_pair_orders,
        &mut runtime,
    ) {
        Ok(success) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(success.layer_order)
        }
        Err(FacewiseAbort::RequiredLayerOrder(error)) => {
            complete_progress(runtime.observer, runtime.work);
            Err(error)
        }
        Err(FacewiseAbort::Impossible(_)) => {
            complete_progress(runtime.observer, runtime.work);
            Err(RequiredLayerOrderError::BaseAnalysisImpossible)
        }
        Err(FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
            reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
        })) => {
            complete_progress(runtime.observer, runtime.work);
            Err(RequiredLayerOrderError::CertificateReverificationFailed)
        }
        Err(FacewiseAbort::Unknown(reason)) => {
            complete_progress(runtime.observer, runtime.work);
            Err(RequiredLayerOrderError::Inconclusive { reason })
        }
        Err(FacewiseAbort::Execution(error)) => Err(RequiredLayerOrderError::Execution(error)),
    }
}

fn solve_facewise<O: GlobalFlatFoldabilityObserver + ?Sized>(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    canonical_faces: &[LayerFace],
    provenance: GlobalFlatFoldabilityProvenance,
    required_pair_orders: Option<&[RequiredLayerOrderPair]>,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<SolveSuccess> {
    runtime.advance(
        GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
        Some(canonical_faces.len()),
    )?;
    let embedding =
        build_flat_embedding(paper, crease_pattern, topology, canonical_faces, runtime)?;
    runtime.advance(GlobalFlatFoldabilityPhase::BuildingOverlapArrangement, None)?;
    let overlap_pairs = build_overlap_pairs(&embedding.faces, runtime)?;
    runtime.set_overlap_pairs(overlap_pairs.len())?;
    let cells = build_overlap_cells(&embedding.faces, &overlap_pairs, runtime)?;
    runtime.set_overlap_cells(cells.len())?;
    solve_layer_order(
        embedding,
        overlap_pairs,
        cells,
        provenance,
        required_pair_orders,
        runtime,
    )
}

fn build_flat_embedding<O: GlobalFlatFoldabilityObserver + ?Sized>(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    canonical_faces: &[LayerFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<FlatEmbedding> {
    let mut vertex_records = crease_pattern.vertices.iter().collect::<Vec<_>>();
    vertex_records.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    let mut vertices = HashMap::with_capacity(vertex_records.len());
    for vertex in vertex_records {
        runtime.checkpoint(None)?;
        if vertices.contains_key(&vertex.id) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DuplicateSourceVertex,
                },
            ));
        }
        let point = point_from_binary64(vertex.position.x, vertex.position.y, runtime)?;
        runtime.add_embedding_exact_storage(exact_storage_bytes_point(&point)?)?;
        vertices.insert(vertex.id, point);
    }
    for boundary_vertex in &paper.boundary_vertices {
        if !vertices.contains_key(boundary_vertex) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                },
            ));
        }
    }

    let mut edge_records = crease_pattern.edges.iter().collect::<Vec<_>>();
    edge_records.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    let mut edges = HashMap::with_capacity(edge_records.len());
    for edge in edge_records {
        runtime.checkpoint(None)?;
        if edge.kind == EdgeKind::Cut {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::CutEdge,
                },
            ));
        }
        if !vertices.contains_key(&edge.start) || !vertices.contains_key(&edge.end) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                },
            ));
        }
        if edges
            .insert(
                edge.id,
                SourceEdge {
                    start: edge.start,
                    end: edge.end,
                    kind: edge.kind,
                },
            )
            .is_some()
        {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DuplicateSourceEdge,
                },
            ));
        }
    }

    let topology_faces = topology
        .faces
        .iter()
        .map(|face| (face.id, face))
        .collect::<HashMap<_, _>>();
    let mut source_faces = Vec::with_capacity(canonical_faces.len());
    let mut face_edge_counts = HashMap::<EdgeId, usize>::new();
    for layer in canonical_faces {
        runtime.checkpoint(None)?;
        let Some(face) = topology_faces.get(&layer.face_id).copied() else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
                },
            ));
        };
        if face.outer.half_edges.len() < 3 {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                },
            ));
        }
        let mut vertex_ids = Vec::with_capacity(face.outer.half_edges.len());
        let mut polygon = Vec::with_capacity(face.outer.half_edges.len());
        let mut unique_vertices = HashSet::with_capacity(face.outer.half_edges.len());
        for (index, half_edge) in face.outer.half_edges.iter().enumerate() {
            let next = &face.outer.half_edges[(index + 1) % face.outer.half_edges.len()];
            if half_edge.destination != next.origin || !unique_vertices.insert(half_edge.origin) {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                    },
                ));
            }
            let Some(source_edge) = edges.get(&half_edge.edge) else {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::MissingSourceEdge,
                    },
                ));
            };
            let observed = face_edge_counts
                .get(&half_edge.edge)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(internal_abort)?;
            face_edge_counts.insert(half_edge.edge, observed);
            let matches_source = (source_edge.start == half_edge.origin
                && source_edge.end == half_edge.destination)
                || (source_edge.end == half_edge.origin
                    && source_edge.start == half_edge.destination);
            if !matches_source {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::InconsistentSourceBoundary,
                    },
                ));
            }
            let Some(point) = vertices.get(&half_edge.origin) else {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                    },
                ));
            };
            runtime.add_embedding_exact_storage(exact_storage_bytes_point(point)?)?;
            vertex_ids.push(half_edge.origin);
            polygon.push(point.clone());
        }
        validate_convex_face(*layer, &polygon, runtime)?;
        source_faces.push(SourceFace {
            layer: *layer,
            vertex_ids,
            source_polygon: polygon,
        });
    }
    let hinge_edges = topology
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<HashSet<_>>();
    let mut material_internal_edge_count = 0_usize;
    for (edge_id, incidence_count) in &face_edge_counts {
        let source_edge = edges.get(edge_id).ok_or_else(internal_abort)?;
        match *incidence_count {
            1 if source_edge.kind == EdgeKind::Boundary => {}
            2 if matches!(source_edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                && hinge_edges.contains(edge_id) =>
            {
                material_internal_edge_count = material_internal_edge_count
                    .checked_add(1)
                    .ok_or_else(internal_abort)?;
            }
            _ => {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                    },
                ));
            }
        }
    }
    if material_internal_edge_count != topology.hinge_adjacency.len() {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
            },
        ));
    }

    let face_indexes = source_faces
        .iter()
        .enumerate()
        .map(|(index, face)| (face.layer.face_id, index))
        .collect::<HashMap<_, _>>();
    let mut adjacency = vec![Vec::new(); source_faces.len()];
    for hinge in &topology.hinge_adjacency {
        let Some(&first) = face_indexes.get(&hinge.first) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
                },
            ));
        };
        let Some(&second) = face_indexes.get(&hinge.second) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
                },
            ));
        };
        let Some(edge) = edges.get(&hinge.edge) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceEdge,
                },
            ));
        };
        if !matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                },
            ));
        }
        let assignment = match edge.kind {
            EdgeKind::Mountain => FoldAssignment::Mountain,
            EdgeKind::Valley => FoldAssignment::Valley,
            EdgeKind::Auxiliary | EdgeKind::Boundary | EdgeKind::Cut => {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                    },
                ));
            }
        };
        if assignment != hinge.assignment {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                },
            ));
        }
        adjacency[first].push((hinge.edge, second, assignment, edge.clone()));
        adjacency[second].push((hinge.edge, first, assignment, edge.clone()));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(edge, neighbor, _, _)| {
            (
                source_faces[*neighbor].layer.face_key,
                edge.canonical_bytes(),
            )
        });
    }

    let reference_face = 0;
    let mut transforms = vec![None::<(Transform, bool, Option<EdgeId>)>; source_faces.len()];
    let identity = Transform::identity();
    runtime.add_embedding_exact_storage(exact_storage_bytes_transform(&identity)?)?;
    transforms[reference_face] = Some((identity, true, None));
    let mut queue = VecDeque::from([reference_face]);
    while let Some(face_index) = queue.pop_front() {
        runtime.checkpoint(None)?;
        let (transform, front_up, _) = transforms[face_index].clone().ok_or_else(internal_abort)?;
        let transform_transient_bytes = exact_storage_bytes_transform(&transform)?;
        runtime.ensure_transient_exact_storage(transform_transient_bytes)?;
        for (edge_id, neighbor, _, edge) in &adjacency[face_index] {
            let source_first = vertices.get(&edge.start).ok_or({
                FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                })
            })?;
            let source_second = vertices.get(&edge.end).ok_or({
                FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                })
            })?;
            let folded_first = apply(&transform, source_first, runtime)?;
            let folded_first_bytes = exact_storage_bytes_point(&folded_first)?;
            runtime.ensure_transient_exact_storage(
                transform_transient_bytes.saturating_add(folded_first_bytes),
            )?;
            let folded_second = apply(&transform, source_second, runtime)?;
            let folded_axis_bytes =
                folded_first_bytes.saturating_add(exact_storage_bytes_point(&folded_second)?);
            runtime.ensure_transient_exact_storage(
                transform_transient_bytes.saturating_add(folded_axis_bytes),
            )?;
            let reflection = reflection_across(&folded_first, &folded_second, runtime)?;
            let reflection_bytes = exact_storage_bytes_transform(&reflection)?;
            let transient_before_candidate = transform_transient_bytes
                .saturating_add(folded_axis_bytes)
                .saturating_add(reflection_bytes);
            runtime.ensure_transient_exact_storage(transient_before_candidate)?;
            let candidate = compose(&reflection, &transform, runtime)?;
            let candidate_bytes = exact_storage_bytes_transform(&candidate)?;
            runtime.ensure_transient_exact_storage(
                transient_before_candidate.saturating_add(candidate_bytes),
            )?;
            let candidate_front_up = !front_up;
            if let Some((existing, existing_front_up, _)) = &transforms[*neighbor] {
                if *existing_front_up != candidate_front_up {
                    return Err(embedding_contradiction(
                        &source_faces[*neighbor],
                        *edge_id,
                        source_faces[*neighbor].vertex_ids[0],
                    ));
                }
                for (vertex_id, point) in source_faces[*neighbor]
                    .vertex_ids
                    .iter()
                    .zip(&source_faces[*neighbor].source_polygon)
                {
                    let existing_point = apply(existing, point, runtime)?;
                    let existing_point_bytes = exact_storage_bytes_point(&existing_point)?;
                    runtime.ensure_transient_exact_storage(
                        transient_before_candidate
                            .saturating_add(candidate_bytes)
                            .saturating_add(existing_point_bytes),
                    )?;
                    let candidate_point = apply(&candidate, point, runtime)?;
                    let comparison_point_bytes = existing_point_bytes
                        .saturating_add(exact_storage_bytes_point(&candidate_point)?);
                    runtime.ensure_transient_exact_storage(
                        transient_before_candidate
                            .saturating_add(candidate_bytes)
                            .saturating_add(comparison_point_bytes),
                    )?;
                    if existing_point != candidate_point {
                        return Err(embedding_contradiction(
                            &source_faces[*neighbor],
                            *edge_id,
                            *vertex_id,
                        ));
                    }
                }
            } else {
                runtime.add_embedding_exact_storage(candidate_bytes)?;
                runtime.ensure_transient_exact_storage(transient_before_candidate)?;
                transforms[*neighbor] = Some((candidate, candidate_front_up, Some(*edge_id)));
                queue.push_back(*neighbor);
            }
        }
    }
    if transforms.iter().any(Option::is_none) {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
            },
        ));
    }

    let mut folded_faces = Vec::with_capacity(source_faces.len());
    for (source, transform) in source_faces.into_iter().zip(transforms) {
        runtime.checkpoint(None)?;
        let (transform, front_up, _) = transform.ok_or_else(internal_abort)?;
        let mut polygon = Vec::with_capacity(source.source_polygon.len());
        for point in &source.source_polygon {
            let folded_point = apply(&transform, point, runtime)?;
            runtime.add_embedding_exact_storage(exact_storage_bytes_point(&folded_point)?)?;
            polygon.push(folded_point);
        }
        let area = signed_double_area(&polygon, runtime)?;
        if area.is_negative() {
            polygon.reverse();
        } else if area.is_zero() {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                },
            ));
        }
        folded_faces.push(FoldedFace {
            source,
            transform,
            front_up,
            polygon,
        });
    }

    let mut folded_hinges = Vec::with_capacity(topology.hinge_adjacency.len());
    for hinge in &topology.hinge_adjacency {
        runtime.checkpoint(None)?;
        let first_face = face_indexes[&hinge.first];
        let second_face = face_indexes[&hinge.second];
        let edge = &edges[&hinge.edge];
        let source_first = &vertices[&edge.start];
        let source_second = &vertices[&edge.end];
        let first_point = apply(&folded_faces[first_face].transform, source_first, runtime)?;
        let first_point_bytes = exact_storage_bytes_point(&first_point)?;
        runtime.ensure_transient_exact_storage(first_point_bytes)?;
        let second_point = apply(&folded_faces[first_face].transform, source_second, runtime)?;
        let stored_hinge_bytes =
            first_point_bytes.saturating_add(exact_storage_bytes_point(&second_point)?);
        runtime.ensure_transient_exact_storage(stored_hinge_bytes)?;
        let other_first = apply(&folded_faces[second_face].transform, source_first, runtime)?;
        let other_first_bytes = exact_storage_bytes_point(&other_first)?;
        runtime
            .ensure_transient_exact_storage(stored_hinge_bytes.saturating_add(other_first_bytes))?;
        let other_second = apply(&folded_faces[second_face].transform, source_second, runtime)?;
        let comparison_bytes =
            other_first_bytes.saturating_add(exact_storage_bytes_point(&other_second)?);
        runtime
            .ensure_transient_exact_storage(stored_hinge_bytes.saturating_add(comparison_bytes))?;
        if first_point != other_first || second_point != other_second {
            return Err(embedding_contradiction(
                &folded_faces[second_face].source,
                hinge.edge,
                edge.start,
            ));
        }
        runtime.add_embedding_exact_storage(stored_hinge_bytes)?;
        runtime.ensure_transient_exact_storage(comparison_bytes)?;
        folded_hinges.push(FoldedHinge {
            edge: hinge.edge,
            first_face,
            second_face,
            assignment: hinge.assignment,
            first_point,
            second_point,
        });
    }
    folded_hinges.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    let embedding = FlatEmbedding {
        reference_face,
        faces: folded_faces,
        hinges: folded_hinges,
        material_internal_edge_count,
    };
    runtime.set_embedding_exact_storage(exact_storage_bytes_embedding(&embedding)?)?;
    Ok(embedding)
}

fn validate_convex_face<O: GlobalFlatFoldabilityObserver + ?Sized>(
    face: LayerFace,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let area = signed_double_area(polygon, runtime)?;
    if area <= Rational::zero() {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::NonConvexFace { face },
        ));
    }
    let mut unique_points = HashSet::with_capacity(polygon.len());
    for point in polygon {
        if !unique_points.insert(point) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                },
            ));
        }
    }
    for index in 0..polygon.len() {
        let turn = cross(
            &polygon[index],
            &polygon[(index + 1) % polygon.len()],
            &polygon[(index + 2) % polygon.len()],
            runtime,
        )?;
        if turn.is_negative() {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::NonConvexFace { face },
            ));
        }
    }
    Ok(())
}

fn embedding_contradiction(face: &SourceFace, edge: EdgeId, vertex: VertexId) -> FacewiseAbort {
    FacewiseAbort::Impossible(
        GlobalFlatFoldabilityImpossibleReason::InconsistentFlatEmbedding {
            face: face.layer,
            conflicting_hinge: edge,
            conflicting_vertex: vertex,
        },
    )
}

fn internal_abort() -> FacewiseAbort {
    FacewiseAbort::Execution(GlobalFlatFoldabilityExecutionError::Internal {
        reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
    })
}

#[derive(Clone)]
struct OverlapPair {
    first: usize,
    second: usize,
}

fn build_overlap_pairs<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapPair>> {
    build_overlap_pairs_with_exact_bounds_pruning(faces, runtime, true)
}

#[cfg(test)]
fn build_overlap_pairs_without_exact_bounds_pruning<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapPair>> {
    build_overlap_pairs_with_exact_bounds_pruning(faces, runtime, false)
}

fn build_overlap_pairs_with_exact_bounds_pruning<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    runtime: &mut Runtime<'_, O>,
    prune_strictly_separated_bounds: bool,
) -> FacewiseResult<Vec<OverlapPair>> {
    let saved_arrangement_bytes = runtime.exact_storage.arrangement_bytes;
    let result = (|| {
        let bounds = if prune_strictly_separated_bounds {
            runtime.checkpoint(None)?;
            let requested_bytes = runtime
                .allocation_bytes(faces.len(), std::mem::size_of::<ExactAxisAlignedBounds>())?;
            runtime.set_arrangement_exact_storage(
                saved_arrangement_bytes
                    .checked_add(requested_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            let mut bounds = Vec::new();
            bounds.try_reserve_exact(faces.len()).map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
            let retained_bytes = runtime.allocation_bytes(
                bounds.capacity(),
                std::mem::size_of::<ExactAxisAlignedBounds>(),
            )?;
            runtime.set_arrangement_exact_storage(
                saved_arrangement_bytes
                    .checked_add(retained_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            for face in faces {
                runtime.checkpoint(None)?;
                bounds.push(exact_axis_aligned_bounds(&face.polygon, runtime)?);
            }
            bounds
        } else {
            Vec::new()
        };
        let mut pairs = Vec::new();
        for first in 0..faces.len() {
            runtime.checkpoint(None)?;
            for second in (first + 1)..faces.len() {
                if prune_strictly_separated_bounds
                    && exact_axis_aligned_bounds_are_strictly_separated(
                        &faces[first].polygon,
                        bounds[first],
                        &faces[second].polygon,
                        bounds[second],
                        runtime,
                    )?
                {
                    continue;
                }
                let intersection = convex_polygon_intersection(
                    &faces[first].polygon,
                    &faces[second].polygon,
                    runtime,
                )?;
                if intersection.len() >= 3 {
                    let area = signed_double_area(&intersection, runtime)?;
                    if area.is_positive() {
                        let observed = pairs.len().checked_add(1).ok_or({
                            FacewiseAbort::Unknown(
                                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                                    resource: FlatFoldabilityResource::OverlapFacePairs,
                                    limit: runtime.limits.max_overlap_face_pairs,
                                    observed: usize::MAX,
                                },
                            )
                        })?;
                        runtime.set_overlap_pairs(observed)?;
                        pairs.push(OverlapPair { first, second });
                    }
                }
            }
        }
        Ok(pairs)
    })();
    runtime.exact_storage.arrangement_bytes = saved_arrangement_bytes;
    result
}

fn convex_polygon_intersection<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &[Point],
    second: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<Point>> {
    let initial_bytes = exact_storage_bytes_points(first)?;
    runtime.ensure_transient_exact_storage(initial_bytes)?;
    let mut output = first.to_vec();
    for edge_index in 0..second.len() {
        if output.is_empty() {
            break;
        }
        let clip_first = &second[edge_index];
        let clip_second = &second[(edge_index + 1) % second.len()];
        let input = std::mem::take(&mut output);
        let input_bytes = exact_storage_bytes_points(&input)?;
        let mut output_bytes = 0_usize;
        let mut previous = input.last().ok_or_else(internal_abort)?;
        let mut previous_side = cross(clip_first, clip_second, previous, runtime)?;
        for current in &input {
            let current_side = cross(clip_first, clip_second, current, runtime)?;
            let previous_inside = !previous_side.is_negative();
            let current_inside = !current_side.is_negative();
            if previous_inside != current_inside {
                let denominator = sub(&previous_side, &current_side, runtime)?;
                let parameter = div(&previous_side, &denominator, runtime)?;
                let intersection = interpolate(previous, current, &parameter, runtime)?;
                push_exact_point_bounded(
                    &mut output,
                    intersection,
                    &mut output_bytes,
                    input_bytes,
                    runtime,
                )?;
            }
            if current_inside {
                push_exact_point_bounded(
                    &mut output,
                    current.clone(),
                    &mut output_bytes,
                    input_bytes,
                    runtime,
                )?;
            }
            previous = current;
            previous_side = current_side;
        }
        deduplicate_polygon(&mut output);
    }
    if output.len() >= 3 && signed_double_area(&output, runtime)?.is_negative() {
        output.reverse();
    }
    Ok(output)
}

fn push_exact_point_bounded<O: GlobalFlatFoldabilityObserver + ?Sized>(
    target: &mut Vec<Point>,
    point: Point,
    target_bytes: &mut usize,
    other_transient_bytes: usize,
    runtime: &Runtime<'_, O>,
) -> FacewiseResult<()> {
    let point_bytes = exact_storage_bytes_point(&point)?;
    let next_target_bytes = target_bytes.saturating_add(point_bytes);
    runtime
        .ensure_transient_exact_storage(other_transient_bytes.saturating_add(next_target_bytes))?;
    target.push(point);
    *target_bytes = next_target_bytes;
    Ok(())
}

fn interpolate<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &Point,
    second: &Point,
    parameter: &Rational,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Point> {
    let dx = sub(&second.x, &first.x, runtime)?;
    let dy = sub(&second.y, &first.y, runtime)?;
    Ok(Point {
        x: add(&first.x, &mul(parameter, &dx, runtime)?, runtime)?,
        y: add(&first.y, &mul(parameter, &dy, runtime)?, runtime)?,
    })
}

fn deduplicate_polygon(polygon: &mut Vec<Point>) {
    polygon.dedup();
    if polygon.len() > 1 && polygon.first() == polygon.last() {
        polygon.pop();
    }
}

#[derive(Clone)]
struct OverlapCell {
    key: OverlapCellKey,
    boundary: Vec<Point>,
    covering_faces: Vec<usize>,
}

struct ArrangementRegion {
    boundary: Vec<Point>,
    /// Sorted superset of faces whose interiors can still meet this region.
    /// `None` is retained only by the test-only unpropagated baseline.
    possible_faces: Option<Vec<usize>>,
}

struct PreparedSupportingLine {
    face_index: usize,
    edge_index: usize,
    canonical_ordinal: usize,
    face_masks: Vec<u8>,
    face_mask_bytes: usize,
    globally_supporting: bool,
}

#[derive(Clone, Copy)]
struct ExactAxisAlignedBounds {
    min_x_point: usize,
    max_x_point: usize,
    min_y_point: usize,
    max_y_point: usize,
}

fn exact_axis_aligned_bounds<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<ExactAxisAlignedBounds> {
    if polygon.is_empty() {
        return Err(certificate_failure());
    }
    let mut bounds = ExactAxisAlignedBounds {
        min_x_point: 0,
        max_x_point: 0,
        min_y_point: 0,
        max_y_point: 0,
    };
    for point in 1..polygon.len() {
        if cmp(&polygon[point].x, &polygon[bounds.min_x_point].x, runtime)? == Ordering::Less {
            bounds.min_x_point = point;
        }
        if cmp(&polygon[point].x, &polygon[bounds.max_x_point].x, runtime)? == Ordering::Greater {
            bounds.max_x_point = point;
        }
        if cmp(&polygon[point].y, &polygon[bounds.min_y_point].y, runtime)? == Ordering::Less {
            bounds.min_y_point = point;
        }
        if cmp(&polygon[point].y, &polygon[bounds.max_y_point].y, runtime)? == Ordering::Greater {
            bounds.max_y_point = point;
        }
    }
    Ok(bounds)
}

fn exact_axis_aligned_bounds_are_strictly_separated<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &[Point],
    first_bounds: ExactAxisAlignedBounds,
    second: &[Point],
    second_bounds: ExactAxisAlignedBounds,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    Ok(cmp(
        &first[first_bounds.max_x_point].x,
        &second[second_bounds.min_x_point].x,
        runtime,
    )? == Ordering::Less
        || cmp(
            &second[second_bounds.max_x_point].x,
            &first[first_bounds.min_x_point].x,
            runtime,
        )? == Ordering::Less
        || cmp(
            &first[first_bounds.max_y_point].y,
            &second[second_bounds.min_y_point].y,
            runtime,
        )? == Ordering::Less
        || cmp(
            &second[second_bounds.max_y_point].y,
            &first[first_bounds.min_y_point].y,
            runtime,
        )? == Ordering::Less)
}

fn exact_axis_aligned_bounds_interiors_cannot_overlap<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &[Point],
    first_bounds: ExactAxisAlignedBounds,
    second: &[Point],
    second_bounds: ExactAxisAlignedBounds,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    Ok(cmp(
        &first[first_bounds.max_x_point].x,
        &second[second_bounds.min_x_point].x,
        runtime,
    )? != Ordering::Greater
        || cmp(
            &second[second_bounds.max_x_point].x,
            &first[first_bounds.min_x_point].x,
            runtime,
        )? != Ordering::Greater
        || cmp(
            &first[first_bounds.max_y_point].y,
            &second[second_bounds.min_y_point].y,
            runtime,
        )? != Ordering::Greater
        || cmp(
            &second[second_bounds.max_y_point].y,
            &first[first_bounds.min_y_point].y,
            runtime,
        )? != Ordering::Greater)
}

fn exact_point_is_strictly_outside_axis_aligned_bounds<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    point: &Point,
    polygon: &[Point],
    bounds: ExactAxisAlignedBounds,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    Ok(
        cmp(&point.x, &polygon[bounds.min_x_point].x, runtime)? == Ordering::Less
            || cmp(&point.x, &polygon[bounds.max_x_point].x, runtime)? == Ordering::Greater
            || cmp(&point.y, &polygon[bounds.min_y_point].y, runtime)? == Ordering::Less
            || cmp(&point.y, &polygon[bounds.max_y_point].y, runtime)? == Ordering::Greater,
    )
}

fn convex_polygon_contains_polygon<O: GlobalFlatFoldabilityObserver + ?Sized>(
    container: &[Point],
    candidate: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    if container.len() < 3 || candidate.len() < 3 {
        return Ok(false);
    }
    let mut point_poll = 0_usize;
    for edge in 0..container.len() {
        let first = &container[edge];
        let second = &container[(edge + 1) % container.len()];
        for point in candidate {
            runtime.poll_control(&mut point_poll)?;
            if cross(first, second, point, runtime)?.is_negative() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn convex_polygon_interiors_overlap<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &[Point],
    second: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    if first.len() < 3 || second.len() < 3 {
        return Ok(false);
    }
    // Both polygons are positive-area, convex, and counter-clockwise at every
    // production call site. Their interiors overlap exactly when each
    // supporting edge half-plane of either polygon contains a strictly-left
    // vertex of the other polygon. Absence of such a vertex proves separation
    // or boundary-only contact without constructing clipped rationals.
    for (supporting_polygon, candidate) in [(first, second), (second, first)] {
        for edge in 0..supporting_polygon.len() {
            runtime.checkpoint(None)?;
            let axis_first = &supporting_polygon[edge];
            let axis_second = &supporting_polygon[(edge + 1) % supporting_polygon.len()];
            let mut has_strictly_left_vertex = false;
            let mut point_poll = 0_usize;
            for point in candidate {
                runtime.poll_control(&mut point_poll)?;
                if cross(axis_first, axis_second, point, runtime)?.is_positive() {
                    has_strictly_left_vertex = true;
                    break;
                }
            }
            if !has_strictly_left_vertex {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn consistent_polygon_class_coverage<O: GlobalFlatFoldabilityObserver + ?Sized>(
    covering_faces: &[usize],
    polygon_classes: &[usize],
    class_index: usize,
    representative: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    if polygon_classes.get(representative) != Some(&class_index) {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let representative_is_covering = covering_faces.binary_search(&representative).is_ok();
    let mut class_poll = 0_usize;
    for (face_index, &candidate_class) in polygon_classes.iter().enumerate() {
        runtime.poll_control(&mut class_poll)?;
        if candidate_class == class_index
            && covering_faces.binary_search(&face_index).is_ok() != representative_is_covering
        {
            return Err(certificate_failure());
        }
    }
    Ok(representative_is_covering)
}

fn build_verifier_face_bounds<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<ExactAxisAlignedBounds>> {
    runtime.checkpoint(None)?;
    let verification_base = runtime.verification_storage_bytes();
    let requested_bytes =
        runtime.allocation_bytes(faces.len(), std::mem::size_of::<ExactAxisAlignedBounds>())?;
    runtime.add_verification_storage(requested_bytes)?;
    let bounds = (|| {
        let mut bounds = Vec::new();
        bounds.try_reserve_exact(faces.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        let retained_bytes = runtime.allocation_bytes(
            bounds.capacity(),
            std::mem::size_of::<ExactAxisAlignedBounds>(),
        )?;
        runtime.restore_verification_storage(verification_base);
        runtime.add_verification_storage(retained_bytes)?;
        for face in faces {
            runtime.checkpoint(None)?;
            bounds.push(exact_axis_aligned_bounds(&face.polygon, runtime)?);
        }
        Ok(bounds)
    })();
    if bounds.is_err() {
        runtime.restore_verification_storage(verification_base);
    }
    bounds
}

fn supporting_lines_share_exact_line<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    first: (usize, usize),
    second: (usize, usize),
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    let first_start = &faces[first.0].polygon[first.1];
    let first_end = &faces[first.0].polygon[(first.1 + 1) % faces[first.0].polygon.len()];
    let second_start = &faces[second.0].polygon[second.1];
    let second_end = &faces[second.0].polygon[(second.1 + 1) % faces[second.0].polygon.len()];
    Ok(
        cross(first_start, first_end, second_start, runtime)?.is_zero()
            && cross(first_start, first_end, second_end, runtime)?.is_zero(),
    )
}

fn supporting_line_keep_flags<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    supporting_lines: &[(usize, usize)],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<(Vec<u8>, usize)> {
    // Preserve control priority before reserving the short-lived
    // deduplication buffer. The caller consumes and drops it before any later
    // arrangement allocation, so a transient peak check is sufficient.
    runtime.checkpoint(None)?;
    let requested_flag_bytes =
        runtime.allocation_bytes(supporting_lines.len(), std::mem::size_of::<u8>())?;
    runtime.ensure_transient_exact_storage(requested_flag_bytes)?;
    let mut flags = Vec::new();
    flags
        .try_reserve_exact(supporting_lines.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let flag_bytes = runtime.allocation_bytes(flags.capacity(), std::mem::size_of::<u8>())?;
    runtime.ensure_transient_exact_storage(flag_bytes)?;
    let mut comparison_poll = 0_usize;
    for (candidate_index, candidate) in supporting_lines.iter().copied().enumerate() {
        runtime.checkpoint(None)?;
        let mut keep = true;
        for prior_index in 0..candidate_index {
            runtime.poll_control(&mut comparison_poll)?;
            if flags[prior_index] != 0
                && supporting_lines_share_exact_line(
                    faces,
                    supporting_lines[prior_index],
                    candidate,
                    runtime,
                )?
            {
                keep = false;
                break;
            }
        }
        flags.push(u8::from(keep));
    }
    Ok((flags, flag_bytes))
}

fn supporting_line_face_interior_masks<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    line_first: &Point,
    line_second: &Point,
    retained_transient_bytes: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<(Vec<u8>, usize)> {
    let requested_bytes = runtime.allocation_bytes(faces.len(), std::mem::size_of::<u8>())?;
    runtime.ensure_transient_exact_storage(
        retained_transient_bytes
            .checked_add(requested_bytes)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    let mut masks = Vec::new();
    masks
        .try_reserve_exact(faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let retained_bytes = runtime.allocation_bytes(masks.capacity(), std::mem::size_of::<u8>())?;
    runtime.ensure_transient_exact_storage(
        retained_transient_bytes
            .checked_add(retained_bytes)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    for face in faces {
        runtime.checkpoint(None)?;
        let mut mask = 0_u8;
        let mut point_poll = 0_usize;
        for point in &face.polygon {
            runtime.poll_control(&mut point_poll)?;
            match exact_side_sign(&cross(line_first, line_second, point, runtime)?) {
                Ordering::Greater => mask |= FACE_INTERIOR_LEFT,
                Ordering::Less => mask |= FACE_INTERIOR_RIGHT,
                Ordering::Equal => {}
            }
        }
        // A positive-area convex face can meet a closed child half-plane in
        // positive area only when at least one of its vertices is strictly in
        // that child's open half-plane. Boundary-only contact is deliberately
        // absent from the mask and cannot cover a positive-area region.
        masks.push(mask);
    }
    Ok((masks, retained_bytes))
}

fn possible_face_storage_bytes<O: GlobalFlatFoldabilityObserver + ?Sized>(
    possible_faces: &Vec<usize>,
    runtime: &Runtime<'_, O>,
) -> FacewiseResult<usize> {
    runtime.allocation_bytes(possible_faces.capacity(), std::mem::size_of::<usize>())
}

fn normalize_covering_face_capacity<O: GlobalFlatFoldabilityObserver + ?Sized>(
    possible_faces: Vec<usize>,
    face_count: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<usize>> {
    let mut target_capacity = 0_usize;
    while target_capacity < possible_faces.len() {
        target_capacity =
            next_vector_capacity(target_capacity, target_capacity, face_count, runtime)?;
    }
    if possible_faces.capacity() == target_capacity {
        return Ok(possible_faces);
    }
    let old_bytes = possible_face_storage_bytes(&possible_faces, runtime)?;
    let new_bytes = runtime.allocation_bytes(target_capacity, std::mem::size_of::<usize>())?;
    runtime.ensure_transient_exact_storage(
        old_bytes
            .checked_add(new_bytes)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(target_capacity)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    normalized.extend(possible_faces);
    Ok(normalized)
}

fn retain_possible_faces_for_side<O: GlobalFlatFoldabilityObserver + ?Sized>(
    possible_faces: &mut Vec<usize>,
    face_masks: &[u8],
    side_mask: u8,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let mut write = 0_usize;
    let mut face_poll = 0_usize;
    for read in 0..possible_faces.len() {
        runtime.poll_control(&mut face_poll)?;
        let face_index = possible_faces[read];
        let Some(mask) = face_masks.get(face_index) else {
            return Err(FacewiseAbort::Execution(internal_error()));
        };
        if mask & side_mask != 0 {
            possible_faces[write] = face_index;
            write += 1;
        }
    }
    possible_faces.truncate(write);
    Ok(())
}

fn split_possible_faces_for_line<O: GlobalFlatFoldabilityObserver + ?Sized>(
    possible_faces: Vec<usize>,
    face_masks: &[u8],
    reused_input: ReusedSplitInput,
    retained_transient_bytes: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<(Vec<usize>, Vec<usize>, usize)> {
    match reused_input {
        ReusedSplitInput::Left | ReusedSplitInput::Right => {
            let (side_mask, left_side) = if reused_input == ReusedSplitInput::Left {
                (FACE_INTERIOR_LEFT, true)
            } else {
                (FACE_INTERIOR_RIGHT, false)
            };
            let mut retained = possible_faces;
            retain_possible_faces_for_side(&mut retained, face_masks, side_mask, runtime)?;
            let retained_bytes = possible_face_storage_bytes(&retained, runtime)?;
            runtime.ensure_transient_exact_storage(
                retained_transient_bytes
                    .checked_add(retained_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            if left_side {
                Ok((retained, Vec::new(), retained_bytes))
            } else {
                Ok((Vec::new(), retained, retained_bytes))
            }
        }
        ReusedSplitInput::None => {
            let mut left_count = 0_usize;
            let mut right_count = 0_usize;
            let mut count_poll = 0_usize;
            for &face_index in &possible_faces {
                runtime.poll_control(&mut count_poll)?;
                let Some(mask) = face_masks.get(face_index) else {
                    return Err(FacewiseAbort::Execution(internal_error()));
                };
                left_count = left_count.saturating_add(usize::from(mask & FACE_INTERIOR_LEFT != 0));
                right_count =
                    right_count.saturating_add(usize::from(mask & FACE_INTERIOR_RIGHT != 0));
            }
            let requested_bytes = runtime.allocation_bytes(
                left_count
                    .checked_add(right_count)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
                std::mem::size_of::<usize>(),
            )?;
            runtime.ensure_transient_exact_storage(
                retained_transient_bytes
                    .checked_add(requested_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            let mut left = Vec::new();
            left.try_reserve_exact(left_count).map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
            let mut right = Vec::new();
            right.try_reserve_exact(right_count).map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
            let retained_bytes = possible_face_storage_bytes(&left, runtime)?
                .checked_add(possible_face_storage_bytes(&right, runtime)?)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            runtime.ensure_transient_exact_storage(
                retained_transient_bytes
                    .checked_add(retained_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            let mut fill_poll = 0_usize;
            for face_index in possible_faces {
                runtime.poll_control(&mut fill_poll)?;
                let mask = face_masks[face_index];
                if mask & FACE_INTERIOR_LEFT != 0 {
                    left.push(face_index);
                }
                if mask & FACE_INTERIOR_RIGHT != 0 {
                    right.push(face_index);
                }
            }
            Ok((left, right, retained_bytes))
        }
    }
}

fn build_overlap_cells<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapCell>> {
    build_overlap_cells_with_supporting_line_deduplication(
        faces,
        pairs,
        runtime,
        OverlapCellBuildOptions::DEFAULT,
    )
}

#[derive(Clone, Copy)]
struct OverlapCellBuildOptions {
    deduplicate_supporting_lines: bool,
    prune_region_face_bounds: bool,
    reuse_prevalidated_regions: bool,
    propagate_region_face_candidates: bool,
    prioritize_global_supporting_lines: bool,
}

impl OverlapCellBuildOptions {
    const DEFAULT: Self = Self {
        deduplicate_supporting_lines: true,
        prune_region_face_bounds: true,
        reuse_prevalidated_regions: true,
        propagate_region_face_candidates: true,
        prioritize_global_supporting_lines: true,
    };
}

#[cfg(test)]
fn build_overlap_cells_without_supporting_line_deduplication<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapCell>> {
    build_overlap_cells_with_supporting_line_deduplication(
        faces,
        pairs,
        runtime,
        OverlapCellBuildOptions {
            deduplicate_supporting_lines: false,
            ..OverlapCellBuildOptions::DEFAULT
        },
    )
}

#[cfg(test)]
fn build_overlap_cells_without_region_face_bounds_pruning<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapCell>> {
    build_overlap_cells_with_supporting_line_deduplication(
        faces,
        pairs,
        runtime,
        OverlapCellBuildOptions {
            prune_region_face_bounds: false,
            ..OverlapCellBuildOptions::DEFAULT
        },
    )
}

#[cfg(test)]
fn build_overlap_cells_without_prevalidated_region_reuse<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapCell>> {
    build_overlap_cells_with_supporting_line_deduplication(
        faces,
        pairs,
        runtime,
        OverlapCellBuildOptions {
            reuse_prevalidated_regions: false,
            ..OverlapCellBuildOptions::DEFAULT
        },
    )
}

#[cfg(test)]
fn build_overlap_cells_without_region_face_candidate_propagation<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapCell>> {
    build_overlap_cells_with_supporting_line_deduplication(
        faces,
        pairs,
        runtime,
        OverlapCellBuildOptions {
            propagate_region_face_candidates: false,
            ..OverlapCellBuildOptions::DEFAULT
        },
    )
}

#[cfg(test)]
fn build_overlap_cells_without_global_supporting_line_priority<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapCell>> {
    build_overlap_cells_with_supporting_line_deduplication(
        faces,
        pairs,
        runtime,
        OverlapCellBuildOptions {
            prioritize_global_supporting_lines: false,
            ..OverlapCellBuildOptions::DEFAULT
        },
    )
}

fn build_overlap_cells_with_supporting_line_deduplication<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
    options: OverlapCellBuildOptions,
) -> FacewiseResult<Vec<OverlapCell>> {
    let saved_arrangement_bytes = runtime.exact_storage.arrangement_bytes;
    let result = build_overlap_cells_with_supporting_line_deduplication_inner(
        faces, pairs, runtime, options,
    );
    if result.is_err() {
        runtime.exact_storage.arrangement_bytes = saved_arrangement_bytes;
    }
    result
}

fn build_overlap_cells_with_supporting_line_deduplication_inner<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
    options: OverlapCellBuildOptions,
) -> FacewiseResult<Vec<OverlapCell>> {
    let OverlapCellBuildOptions {
        deduplicate_supporting_lines,
        prune_region_face_bounds,
        reuse_prevalidated_regions,
        propagate_region_face_candidates,
        prioritize_global_supporting_lines,
    } = options;
    let mut all_points = faces.iter().flat_map(|face| face.polygon.iter());
    let first = all_points.next().ok_or_else(internal_abort)?;
    let (mut min_x, mut max_x) = (first.x.clone(), first.x.clone());
    let (mut min_y, mut max_y) = (first.y.clone(), first.y.clone());
    for point in all_points {
        if cmp(&point.x, &min_x, runtime)? == Ordering::Less {
            min_x = point.x.clone();
        }
        if cmp(&point.x, &max_x, runtime)? == Ordering::Greater {
            max_x = point.x.clone();
        }
        if cmp(&point.y, &min_y, runtime)? == Ordering::Less {
            min_y = point.y.clone();
        }
        if cmp(&point.y, &max_y, runtime)? == Ordering::Greater {
            max_y = point.y.clone();
        }
    }
    if min_x == max_x || min_y == max_y {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
            },
        ));
    }
    let bounding_rectangle = vec![
        Point {
            x: min_x.clone(),
            y: min_y.clone(),
        },
        Point {
            x: max_x.clone(),
            y: min_y,
        },
        Point {
            x: max_x,
            y: max_y.clone(),
        },
        Point { x: min_x, y: max_y },
    ];
    let mut retained_region_exact_bytes = exact_storage_bytes_points(&bounding_rectangle)?;
    runtime.set_arrangement_exact_storage(retained_region_exact_bytes)?;
    let initial_possible_faces = if propagate_region_face_candidates {
        let allocation = (|| {
            let requested_bytes =
                runtime.allocation_bytes(faces.len(), std::mem::size_of::<usize>())?;
            runtime.set_arrangement_exact_storage(
                retained_region_exact_bytes
                    .checked_add(requested_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            let mut possible_faces = Vec::new();
            possible_faces.try_reserve_exact(faces.len()).map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
            let retained_bytes = possible_face_storage_bytes(&possible_faces, runtime)?;
            runtime.set_arrangement_exact_storage(
                retained_region_exact_bytes
                    .checked_add(retained_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            possible_faces.extend(0..faces.len());
            Ok((possible_faces, retained_bytes))
        })();
        match allocation {
            Ok((possible_faces, retained_bytes)) => (Some(possible_faces), retained_bytes),
            Err(abort) => {
                runtime.exact_storage.arrangement_bytes = retained_region_exact_bytes;
                return Err(abort);
            }
        }
    } else {
        (None, 0_usize)
    };
    let mut retained_region_metadata_bytes = initial_possible_faces.1;
    let mut regions = vec![ArrangementRegion {
        boundary: bounding_rectangle,
        possible_faces: initial_possible_faces.0,
    }];
    let mut supporting_lines = faces
        .iter()
        .enumerate()
        .flat_map(|(face_index, face)| {
            (0..face.polygon.len()).map(move |edge_index| (face_index, edge_index))
        })
        .collect::<Vec<_>>();
    let supporting_line_tuple_bytes = runtime.allocation_bytes(
        supporting_lines.capacity(),
        std::mem::size_of::<(usize, usize)>(),
    )?;
    runtime.set_arrangement_exact_storage(
        retained_region_exact_bytes
            .checked_add(retained_region_metadata_bytes)
            .and_then(|total| total.checked_add(supporting_line_tuple_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    supporting_lines.sort_unstable_by_key(|(face_index, edge_index)| {
        (
            faces[*face_index].source.layer.face_key,
            *edge_index,
            faces[*face_index].source.layer.face_id.canonical_bytes(),
        )
    });
    if deduplicate_supporting_lines {
        let (keep_flags, _) = supporting_line_keep_flags(faces, &supporting_lines, runtime)?;
        let mut line_index = 0_usize;
        supporting_lines.retain(|_| {
            let keep = keep_flags[line_index] != 0;
            line_index += 1;
            keep
        });
    }
    let mut retained_supporting_line_mask_bytes = 0_usize;
    let requested_prepared_line_bytes = runtime.allocation_bytes(
        supporting_lines.len(),
        std::mem::size_of::<PreparedSupportingLine>(),
    )?;
    runtime.set_arrangement_exact_storage(
        retained_region_exact_bytes
            .checked_add(retained_region_metadata_bytes)
            .and_then(|total| total.checked_add(supporting_line_tuple_bytes))
            .and_then(|total| total.checked_add(requested_prepared_line_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    let mut prepared_supporting_lines = Vec::new();
    prepared_supporting_lines
        .try_reserve_exact(supporting_lines.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let prepared_line_structure_bytes = runtime.allocation_bytes(
        prepared_supporting_lines.capacity(),
        std::mem::size_of::<PreparedSupportingLine>(),
    )?;
    runtime.set_arrangement_exact_storage(
        retained_region_exact_bytes
            .checked_add(retained_region_metadata_bytes)
            .and_then(|total| total.checked_add(supporting_line_tuple_bytes))
            .and_then(|total| total.checked_add(prepared_line_structure_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    for (canonical_ordinal, (face_index, edge_index)) in supporting_lines.into_iter().enumerate() {
        runtime.checkpoint(None)?;
        let line_first = &faces[face_index].polygon[edge_index];
        let line_second =
            &faces[face_index].polygon[(edge_index + 1) % faces[face_index].polygon.len()];
        let (face_masks, face_mask_bytes) = if propagate_region_face_candidates {
            supporting_line_face_interior_masks(faces, line_first, line_second, 0, runtime)?
        } else {
            (Vec::new(), 0_usize)
        };
        let globally_supporting = propagate_region_face_candidates
            && (!face_masks.iter().any(|mask| mask & FACE_INTERIOR_LEFT != 0)
                || !face_masks
                    .iter()
                    .any(|mask| mask & FACE_INTERIOR_RIGHT != 0));
        retained_supporting_line_mask_bytes = retained_supporting_line_mask_bytes
            .checked_add(face_mask_bytes)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        runtime.set_arrangement_exact_storage(
            retained_region_exact_bytes
                .checked_add(retained_region_metadata_bytes)
                .and_then(|total| total.checked_add(supporting_line_tuple_bytes))
                .and_then(|total| total.checked_add(prepared_line_structure_bytes))
                .and_then(|total| total.checked_add(retained_supporting_line_mask_bytes))
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        prepared_supporting_lines.push(PreparedSupportingLine {
            face_index,
            edge_index,
            canonical_ordinal,
            face_masks,
            face_mask_bytes,
            globally_supporting,
        });
    }
    // Consuming the canonical tuple buffer above releases it before sorting
    // or traversing the prepared line records.
    runtime.set_arrangement_exact_storage(
        retained_region_exact_bytes
            .checked_add(retained_region_metadata_bytes)
            .and_then(|total| total.checked_add(prepared_line_structure_bytes))
            .and_then(|total| total.checked_add(retained_supporting_line_mask_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    if prioritize_global_supporting_lines && propagate_region_face_candidates {
        // The ordinal preserves the existing canonical tie order without the
        // hidden scratch allocation of a stable sort.
        prepared_supporting_lines
            .sort_unstable_by_key(|line| (!line.globally_supporting, line.canonical_ordinal));
    }
    for supporting_line in prepared_supporting_lines {
        runtime.checkpoint(None)?;
        let PreparedSupportingLine {
            face_index,
            edge_index,
            canonical_ordinal: _,
            face_masks,
            face_mask_bytes,
            globally_supporting: _,
        } = supporting_line;
        let line_first = &faces[face_index].polygon[edge_index];
        let line_second =
            &faces[face_index].polygon[(edge_index + 1) % faces[face_index].polygon.len()];
        let mut next_regions = Vec::new();
        let mut next_region_exact_bytes = 0_usize;
        let mut next_region_metadata_bytes = 0_usize;
        for region in regions {
            runtime.checkpoint(None)?;
            let ArrangementRegion {
                boundary,
                possible_faces,
            } = region;
            let retained_transient_bytes = next_region_exact_bytes
                .checked_add(next_region_metadata_bytes)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            let (left, right, reused_input) = split_owned_convex_polygon_by_line(
                boundary,
                line_first,
                line_second,
                retained_transient_bytes,
                runtime,
            )?;
            let split_boundary_bytes = exact_storage_bytes_points(&left)?
                .checked_add(exact_storage_bytes_points(&right)?)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            let (left_possible_faces, right_possible_faces, split_metadata_bytes) =
                if let Some(possible_faces) = possible_faces {
                    let (left_faces, right_faces, metadata_bytes) = split_possible_faces_for_line(
                        possible_faces,
                        &face_masks,
                        reused_input,
                        retained_transient_bytes
                            .checked_add(split_boundary_bytes)
                            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
                        runtime,
                    )?;
                    (Some(left_faces), Some(right_faces), metadata_bytes)
                } else {
                    (None, None, 0_usize)
                };
            runtime.ensure_transient_exact_storage(
                retained_transient_bytes
                    .checked_add(split_boundary_bytes)
                    .and_then(|total| total.checked_add(split_metadata_bytes))
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            for (side, (mut candidate, candidate_possible_faces)) in
                [(left, left_possible_faces), (right, right_possible_faces)]
                    .into_iter()
                    .enumerate()
            {
                if candidate_possible_faces.as_ref().is_some_and(Vec::is_empty) {
                    continue;
                }
                let candidate_metadata_bytes = candidate_possible_faces
                    .as_ref()
                    .map_or(Ok(0_usize), |possible_faces| {
                        possible_face_storage_bytes(possible_faces, runtime)
                    })?;
                if reuse_prevalidated_regions && reused_input.matches_side(side) {
                    // Every retained region was simplified and proved to have
                    // positive area when it entered the prior generation. A
                    // strict same-side preflight transfers that owned region
                    // unchanged, so repeating both exact passes cannot alter
                    // its boundary or orientation.
                    let candidate_bytes = exact_storage_bytes_points(&candidate)?;
                    next_region_exact_bytes = next_region_exact_bytes
                        .checked_add(candidate_bytes)
                        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
                    next_region_metadata_bytes = next_region_metadata_bytes
                        .checked_add(candidate_metadata_bytes)
                        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
                    runtime.ensure_transient_exact_storage(
                        next_region_exact_bytes
                            .checked_add(next_region_metadata_bytes)
                            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
                    )?;
                    next_regions.push(ArrangementRegion {
                        boundary: candidate,
                        possible_faces: candidate_possible_faces,
                    });
                    continue;
                }
                simplify_convex_polygon(&mut candidate, runtime)?;
                if candidate.len() < 3 {
                    continue;
                }
                if signed_double_area(&candidate, runtime)?.is_positive() {
                    let candidate_bytes = exact_storage_bytes_points(&candidate)?;
                    next_region_exact_bytes = next_region_exact_bytes
                        .checked_add(candidate_bytes)
                        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
                    next_region_metadata_bytes = next_region_metadata_bytes
                        .checked_add(candidate_metadata_bytes)
                        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
                    runtime.ensure_transient_exact_storage(
                        next_region_exact_bytes
                            .checked_add(next_region_metadata_bytes)
                            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
                    )?;
                    next_regions.push(ArrangementRegion {
                        boundary: candidate,
                        possible_faces: candidate_possible_faces,
                    });
                }
            }
        }
        if next_regions.len() > runtime.limits.max_overlap_cells {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                    resource: FlatFoldabilityResource::OverlapCells,
                    limit: runtime.limits.max_overlap_cells,
                    observed: next_regions.len(),
                },
            ));
        }
        let segment_count = next_regions.iter().try_fold(0_usize, |total, region| {
            total.checked_add(region.boundary.len()).ok_or({
                FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                        resource: FlatFoldabilityResource::ArrangementSegments,
                        limit: runtime.limits.max_arrangement_segments,
                        observed: usize::MAX,
                    },
                )
            })
        })?;
        runtime.set_arrangement_segments(segment_count)?;
        drop(face_masks);
        retained_region_exact_bytes = next_region_exact_bytes;
        retained_region_metadata_bytes = next_region_metadata_bytes;
        retained_supporting_line_mask_bytes = retained_supporting_line_mask_bytes
            .checked_sub(face_mask_bytes)
            .ok_or_else(|| FacewiseAbort::Execution(internal_error()))?;
        runtime.set_arrangement_exact_storage(
            retained_region_exact_bytes
                .checked_add(retained_region_metadata_bytes)
                .and_then(|total| total.checked_add(prepared_line_structure_bytes))
                .and_then(|total| total.checked_add(retained_supporting_line_mask_bytes))
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        regions = next_regions;
    }

    if retained_supporting_line_mask_bytes != 0 {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    // The prepared record buffer is released with its consuming iterator.
    runtime.set_arrangement_exact_storage(
        retained_region_exact_bytes
            .checked_add(retained_region_metadata_bytes)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;

    let retained_arrangement_bytes = retained_region_exact_bytes
        .checked_add(retained_region_metadata_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    // Once every retained supporting line has filtered each region's face
    // candidates, that sorted list is the exact positive-area covering set:
    // every convex face contributes all of its edge supporting lines, and a
    // region outside that face is removed by at least one exterior half-plane.
    // Bounds are therefore needed only by the test-only unpropagated baseline.
    let face_bounds = if prune_region_face_bounds && !propagate_region_face_candidates {
        Some(build_arrangement_face_bounds(
            faces,
            retained_arrangement_bytes,
            runtime,
        )?)
    } else {
        None
    };
    let classified =
        classify_overlap_regions(faces, pairs, regions, face_bounds.as_deref(), runtime);
    drop(face_bounds);
    let (cells, cell_boundary_bytes) = match classified {
        Ok(classified) => classified,
        Err(abort) => {
            runtime.exact_storage.arrangement_bytes = retained_arrangement_bytes;
            return Err(abort);
        }
    };
    if let Err(abort) = runtime.set_arrangement_exact_storage(cell_boundary_bytes) {
        runtime.exact_storage.arrangement_bytes = retained_arrangement_bytes;
        return Err(abort);
    }
    Ok(cells)
}

fn build_arrangement_face_bounds<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    retained_region_bytes: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<ExactAxisAlignedBounds>> {
    let bounds = (|| {
        runtime.checkpoint(None)?;
        let requested_bytes =
            runtime.allocation_bytes(faces.len(), std::mem::size_of::<ExactAxisAlignedBounds>())?;
        runtime.set_arrangement_exact_storage(
            retained_region_bytes
                .checked_add(requested_bytes)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        let mut bounds = Vec::new();
        bounds.try_reserve_exact(faces.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        let retained_bounds_bytes = runtime.allocation_bytes(
            bounds.capacity(),
            std::mem::size_of::<ExactAxisAlignedBounds>(),
        )?;
        runtime.set_arrangement_exact_storage(
            retained_region_bytes
                .checked_add(retained_bounds_bytes)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        for face in faces {
            runtime.checkpoint(None)?;
            bounds.push(exact_axis_aligned_bounds(&face.polygon, runtime)?);
        }
        Ok(bounds)
    })();
    if bounds.is_err() {
        runtime.exact_storage.arrangement_bytes = retained_region_bytes;
    }
    bounds
}

fn classify_overlap_regions<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    regions: Vec<ArrangementRegion>,
    face_bounds: Option<&[ExactAxisAlignedBounds]>,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<(Vec<OverlapCell>, usize)> {
    if face_bounds.is_some_and(|bounds| bounds.len() != faces.len()) {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let mut cells = BTreeMap::<[u8; 32], OverlapCell>::new();
    for region in regions {
        runtime.checkpoint(None)?;
        let ArrangementRegion {
            boundary,
            possible_faces,
        } = region;
        let covering_faces = if let Some(possible_faces) = possible_faces {
            // Every positive-area arrangement region lies wholly in one open
            // half-plane of every retained line. Deduplication removes only
            // coincident lines, while the per-line masks are recomputed for
            // every face and orientation. A face survives precisely when the
            // region is on the interior side of each of that face's edge
            // lines, which is exactly convex-polygon interior membership.
            // Preserve the certificate's deterministic covering-vector
            // capacity (and therefore exact byte accounting) from the
            // independent classifier while avoiding all geometric rechecks.
            normalize_covering_face_capacity(possible_faces, faces.len(), runtime)?
        } else {
            // Test-only baseline used to prove the propagated invariant against
            // the independent representative-point classifier.
            let representative = representative_point(&boundary, runtime)?;
            let mut covering_faces = Vec::new();
            let mut covering_face_poll = 0_usize;
            for (index, face) in faces.iter().enumerate() {
                runtime.poll_control(&mut covering_face_poll)?;
                if let Some(bounds) = face_bounds
                    && exact_point_is_strictly_outside_axis_aligned_bounds(
                        &representative,
                        &face.polygon,
                        bounds[index],
                        runtime,
                    )?
                {
                    continue;
                }
                if point_in_convex_polygon(&representative, &face.polygon, runtime)? {
                    if covering_faces.len() == covering_faces.capacity() {
                        let prior_capacity = covering_faces.capacity();
                        let next_capacity = next_vector_capacity(
                            prior_capacity,
                            covering_faces.len(),
                            faces.len(),
                            runtime,
                        )?;
                        let old_bytes = runtime
                            .allocation_bytes(prior_capacity, std::mem::size_of::<usize>())?;
                        let next_bytes = runtime
                            .allocation_bytes(next_capacity, std::mem::size_of::<usize>())?;
                        runtime.ensure_transient_exact_storage(
                            old_bytes
                                .checked_add(next_bytes)
                                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
                        )?;
                        covering_faces
                            .try_reserve_exact(next_capacity - covering_faces.len())
                            .map_err(|_| {
                                runtime.exact_storage_limit_failure(
                                    runtime.limits.max_certificate_bytes,
                                )
                            })?;
                    }
                    covering_faces.push(index);
                }
            }
            drop(representative);
            covering_faces
        };
        if covering_faces.is_empty() {
            continue;
        }
        let key = overlap_cell_key(&boundary, &covering_faces, faces, runtime)?;
        if let std::collections::btree_map::Entry::Vacant(entry) = cells.entry(key.0) {
            let covering_bytes = runtime
                .allocation_bytes(covering_faces.capacity(), std::mem::size_of::<usize>())?;
            let retained_structure_bytes = std::mem::size_of::<OverlapCell>()
                .checked_add(std::mem::size_of::<[u8; 32]>())
                .and_then(|total| total.checked_add(covering_bytes))
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            runtime.add_certificate_structure_storage(retained_structure_bytes)?;
            entry.insert(OverlapCell {
                key,
                boundary,
                covering_faces,
            });
        }
        runtime.set_overlap_cells(cells.len())?;
    }
    let cells = cells.into_values().collect::<Vec<_>>();
    let cell_boundary_bytes = cells.iter().try_fold(0_usize, |total, cell| {
        total
            .checked_add(exact_storage_bytes_points(&cell.boundary)?)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))
    })?;
    for pair in pairs {
        if !cells.iter().any(|cell| {
            cell.covering_faces.contains(&pair.first) && cell.covering_faces.contains(&pair.second)
        }) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
                },
            ));
        }
    }
    Ok((cells, cell_boundary_bytes))
}

fn verify_canonical_overlap_cells<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    cells: &[OverlapCell],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    runtime.checkpoint(None)?;
    let saved_storage = runtime.exact_storage;
    let saved_arrangement_segments = runtime.work.arrangement_segments;
    let saved_overlap_cells = runtime.work.overlap_cells;

    // The supplied arrangement remains live while the verifier reconstructs
    // the canonical arrangement. Move its exact boundary charge into the
    // verifier scope so the second arrangement cannot hide behind the
    // replaceable primary arrangement slot.
    let retained_arrangement = saved_storage
        .verification_bytes
        .checked_add(saved_storage.arrangement_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let mut reconstruction_storage = saved_storage;
    reconstruction_storage.arrangement_bytes = 0;
    reconstruction_storage.verification_bytes = retained_arrangement;
    runtime.ensure_storage_values(reconstruction_storage, 0)?;
    runtime.exact_storage = reconstruction_storage;

    let verification = (|| {
        let canonical = build_overlap_cells(faces, &[], runtime)?;
        if canonical.len() != cells.len() {
            return Err(certificate_failure());
        }

        let ordering_bytes = runtime.allocation_bytes(cells.len(), std::mem::size_of::<usize>())?;
        runtime.add_verification_storage(ordering_bytes)?;
        let mut supplied_order = Vec::new();
        supplied_order.try_reserve_exact(cells.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        supplied_order.extend(0..cells.len());
        supplied_order.sort_unstable_by_key(|index| cells[*index].key.0);

        for (expected, supplied_index) in canonical.iter().zip(supplied_order) {
            runtime.checkpoint(None)?;
            let supplied = &cells[supplied_index];
            if supplied.key != expected.key
                || supplied.boundary != expected.boundary
                || supplied.covering_faces != expected.covering_faces
            {
                return Err(certificate_failure());
            }
        }
        Ok(())
    })();

    // Exact-operation/value counters intentionally retain reconstruction
    // work. Only the temporary arrangement counters and live storage slots
    // return to their pre-verification values.
    runtime.work.arrangement_segments = saved_arrangement_segments;
    runtime.work.overlap_cells = saved_overlap_cells;
    runtime.exact_storage = saved_storage;
    verification
}

fn split_convex_polygon_by_line<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &[Point],
    line_first: &Point,
    line_second: &Point,
    retained_transient_bytes: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<(Vec<Point>, Vec<Point>)> {
    if polygon.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut left_bytes = 0_usize;
    let mut right_bytes = 0_usize;
    let mut previous = polygon.last().ok_or_else(internal_abort)?;
    let mut previous_side = cross(line_first, line_second, previous, runtime)?;
    for current in polygon {
        let current_side = cross(line_first, line_second, current, runtime)?;
        match (
            exact_side_sign(&previous_side),
            exact_side_sign(&current_side),
        ) {
            (Ordering::Greater, Ordering::Greater) => push_exact_point_bounded(
                &mut left,
                current.clone(),
                &mut left_bytes,
                retained_transient_bytes.saturating_add(right_bytes),
                runtime,
            )?,
            (Ordering::Greater, Ordering::Equal)
            | (Ordering::Equal, Ordering::Equal)
            | (Ordering::Less, Ordering::Equal) => push_exact_point_to_both_bounded(
                &mut left,
                &mut right,
                current.clone(),
                &mut left_bytes,
                &mut right_bytes,
                retained_transient_bytes,
                runtime,
            )?,
            (Ordering::Greater, Ordering::Less) | (Ordering::Less, Ordering::Greater) => {
                let denominator = sub(&previous_side, &current_side, runtime)?;
                let parameter = div(&previous_side, &denominator, runtime)?;
                let intersection = interpolate(previous, current, &parameter, runtime)?;
                push_exact_point_to_both_bounded(
                    &mut left,
                    &mut right,
                    intersection,
                    &mut left_bytes,
                    &mut right_bytes,
                    retained_transient_bytes,
                    runtime,
                )?;
                if current_side.is_positive() {
                    push_exact_point_bounded(
                        &mut left,
                        current.clone(),
                        &mut left_bytes,
                        retained_transient_bytes.saturating_add(right_bytes),
                        runtime,
                    )?;
                } else {
                    push_exact_point_bounded(
                        &mut right,
                        current.clone(),
                        &mut right_bytes,
                        retained_transient_bytes.saturating_add(left_bytes),
                        runtime,
                    )?;
                }
            }
            (Ordering::Equal, Ordering::Greater) => {
                push_exact_point_bounded(
                    &mut left,
                    current.clone(),
                    &mut left_bytes,
                    retained_transient_bytes.saturating_add(right_bytes),
                    runtime,
                )?;
                push_exact_point_bounded(
                    &mut right,
                    previous.clone(),
                    &mut right_bytes,
                    retained_transient_bytes.saturating_add(left_bytes),
                    runtime,
                )?;
            }
            (Ordering::Equal, Ordering::Less) => {
                push_exact_point_bounded(
                    &mut left,
                    previous.clone(),
                    &mut left_bytes,
                    retained_transient_bytes.saturating_add(right_bytes),
                    runtime,
                )?;
                push_exact_point_bounded(
                    &mut right,
                    current.clone(),
                    &mut right_bytes,
                    retained_transient_bytes.saturating_add(left_bytes),
                    runtime,
                )?;
            }
            (Ordering::Less, Ordering::Less) => push_exact_point_bounded(
                &mut right,
                current.clone(),
                &mut right_bytes,
                retained_transient_bytes.saturating_add(left_bytes),
                runtime,
            )?,
        }
        previous = current;
        previous_side = current_side;
    }
    deduplicate_polygon(&mut left);
    deduplicate_polygon(&mut right);
    Ok((left, right))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReusedSplitInput {
    None,
    Left,
    Right,
}

impl ReusedSplitInput {
    fn matches_side(self, side: usize) -> bool {
        matches!((self, side), (Self::Left, 0) | (Self::Right, 1))
    }
}

fn split_owned_convex_polygon_by_line<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: Vec<Point>,
    line_first: &Point,
    line_second: &Point,
    retained_transient_bytes: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<(Vec<Point>, Vec<Point>, ReusedSplitInput)> {
    let mut strict_side = None;
    let mut preflight_poll = 0_usize;
    for point in &polygon {
        runtime.poll_control(&mut preflight_poll)?;
        let side = exact_side_sign(&cross(line_first, line_second, point, runtime)?);
        if side == Ordering::Equal || strict_side.is_some_and(|existing| existing != side) {
            let (left, right) = split_convex_polygon_by_line(
                &polygon,
                line_first,
                line_second,
                retained_transient_bytes,
                runtime,
            )?;
            return Ok((left, right, ReusedSplitInput::None));
        }
        strict_side = Some(side);
    }
    match strict_side {
        Some(Ordering::Greater) => Ok((polygon, Vec::new(), ReusedSplitInput::Left)),
        Some(Ordering::Less) => Ok((Vec::new(), polygon, ReusedSplitInput::Right)),
        Some(Ordering::Equal) => Err(FacewiseAbort::Execution(internal_error())),
        None => Ok((Vec::new(), Vec::new(), ReusedSplitInput::None)),
    }
}

fn exact_side_sign(value: &Rational) -> Ordering {
    if value.is_positive() {
        Ordering::Greater
    } else if value.is_negative() {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

fn push_exact_point_to_both_bounded<O: GlobalFlatFoldabilityObserver + ?Sized>(
    left: &mut Vec<Point>,
    right: &mut Vec<Point>,
    point: Point,
    left_bytes: &mut usize,
    right_bytes: &mut usize,
    retained_transient_bytes: usize,
    runtime: &Runtime<'_, O>,
) -> FacewiseResult<()> {
    let point_bytes = exact_storage_bytes_point(&point)?;
    let next_left_bytes = left_bytes.saturating_add(point_bytes);
    let next_right_bytes = right_bytes.saturating_add(point_bytes);
    runtime.ensure_transient_exact_storage(
        retained_transient_bytes
            .saturating_add(next_left_bytes)
            .saturating_add(next_right_bytes),
    )?;
    left.push(point.clone());
    right.push(point);
    *left_bytes = next_left_bytes;
    *right_bytes = next_right_bytes;
    Ok(())
}

#[cfg(test)]
fn clip_polygon_halfplane<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &[Point],
    line_first: &Point,
    line_second: &Point,
    keep_left: bool,
    other_transient_bytes: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<Point>> {
    if polygon.is_empty() {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    let mut output_bytes = 0_usize;
    let mut previous = polygon.last().ok_or_else(internal_abort)?;
    let mut previous_side = cross(line_first, line_second, previous, runtime)?;
    for current in polygon {
        let current_side = cross(line_first, line_second, current, runtime)?;
        let previous_inside = if keep_left {
            !previous_side.is_negative()
        } else {
            !previous_side.is_positive()
        };
        let current_inside = if keep_left {
            !current_side.is_negative()
        } else {
            !current_side.is_positive()
        };
        if previous_inside != current_inside {
            let denominator = sub(&previous_side, &current_side, runtime)?;
            let parameter = div(&previous_side, &denominator, runtime)?;
            let intersection = interpolate(previous, current, &parameter, runtime)?;
            push_exact_point_bounded(
                &mut output,
                intersection,
                &mut output_bytes,
                other_transient_bytes,
                runtime,
            )?;
        }
        if current_inside {
            push_exact_point_bounded(
                &mut output,
                current.clone(),
                &mut output_bytes,
                other_transient_bytes,
                runtime,
            )?;
        }
        previous = current;
        previous_side = current_side;
    }
    deduplicate_polygon(&mut output);
    Ok(output)
}

fn simplify_convex_polygon<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &mut Vec<Point>,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let mut changed = true;
    while changed && polygon.len() >= 3 {
        changed = false;
        for index in 0..polygon.len() {
            let previous = (index + polygon.len() - 1) % polygon.len();
            let next = (index + 1) % polygon.len();
            if cross(&polygon[previous], &polygon[index], &polygon[next], runtime)?.is_zero() {
                polygon.remove(index);
                changed = true;
                break;
            }
        }
    }
    Ok(())
}

fn representative_point<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Point> {
    if polygon.len() < 3 {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
            },
        ));
    }
    for index in 0..polygon.len() {
        let first = &polygon[(index + polygon.len() - 1) % polygon.len()];
        let second = &polygon[index];
        let third = &polygon[(index + 1) % polygon.len()];
        if cross(first, second, third, runtime)?.is_positive() {
            let candidate = average3(first, second, third, runtime)?;
            if point_in_simple_polygon(&candidate, polygon, runtime)? {
                return Ok(candidate);
            }
        }
    }
    Err(FacewiseAbort::Unknown(
        GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
            reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
        },
    ))
}

fn point_in_convex_polygon<O: GlobalFlatFoldabilityObserver + ?Sized>(
    point: &Point,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    for index in 0..polygon.len() {
        let side = cross(
            &polygon[index],
            &polygon[(index + 1) % polygon.len()],
            point,
            runtime,
        )?;
        if !side.is_positive() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn point_in_simple_polygon<O: GlobalFlatFoldabilityObserver + ?Sized>(
    point: &Point,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    let mut winding = 0_i32;
    for index in 0..polygon.len() {
        let first = &polygon[index];
        let second = &polygon[(index + 1) % polygon.len()];
        let side = cross(first, second, point, runtime)?;
        if side.is_zero() && point_in_segment_bounds(point, first, second, runtime)? {
            return Ok(false);
        }
        let first_below = cmp(&first.y, &point.y, runtime)? != Ordering::Greater;
        let second_below = cmp(&second.y, &point.y, runtime)? != Ordering::Greater;
        if first_below && !second_below && side.is_positive() {
            winding += 1;
        } else if !first_below && second_below && side.is_negative() {
            winding -= 1;
        }
    }
    Ok(winding != 0)
}

fn point_in_segment_bounds<O: GlobalFlatFoldabilityObserver + ?Sized>(
    point: &Point,
    first: &Point,
    second: &Point,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    let min_x = if cmp(&first.x, &second.x, runtime)? == Ordering::Greater {
        &second.x
    } else {
        &first.x
    };
    let max_x = if cmp(&first.x, &second.x, runtime)? == Ordering::Greater {
        &first.x
    } else {
        &second.x
    };
    let min_y = if cmp(&first.y, &second.y, runtime)? == Ordering::Greater {
        &second.y
    } else {
        &first.y
    };
    let max_y = if cmp(&first.y, &second.y, runtime)? == Ordering::Greater {
        &first.y
    } else {
        &second.y
    };
    Ok(cmp(&point.x, min_x, runtime)? != Ordering::Less
        && cmp(&point.x, max_x, runtime)? != Ordering::Greater
        && cmp(&point.y, min_y, runtime)? != Ordering::Less
        && cmp(&point.y, max_y, runtime)? != Ordering::Greater)
}

fn overlap_cell_key<O: GlobalFlatFoldabilityObserver + ?Sized>(
    boundary: &[Point],
    covering_faces: &[usize],
    faces: &[FoldedFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<OverlapCellKey> {
    runtime.checkpoint(None)?;
    let encoded_headers =
        runtime.allocation_bytes(boundary.len(), std::mem::size_of::<Vec<u8>>())?;
    let canonical_transient = exact_storage_bytes_points(boundary)?
        .checked_add(encoded_headers)
        .and_then(|total| total.checked_add(std::mem::size_of::<Vec<Vec<u8>>>()))
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.ensure_transient_exact_storage(canonical_transient)?;
    let canonical = canonical_boundary_bytes(boundary, runtime)?;
    let mut hasher = Sha256::new();
    hasher.update(CELL_KEY_DOMAIN);
    hasher.update(
        u64::try_from(boundary.len())
            .map_err(|_| FacewiseAbort::Execution(internal_error()))?
            .to_be_bytes(),
    );
    let mut hash_poll = 0_usize;
    for point in canonical {
        runtime.poll_control(&mut hash_poll)?;
        hasher.update(
            u64::try_from(point.len())
                .map_err(|_| FacewiseAbort::Execution(internal_error()))?
                .to_be_bytes(),
        );
        hasher.update(point);
    }
    for face in covering_faces {
        runtime.poll_control(&mut hash_poll)?;
        hasher.update(faces[*face].source.layer.face_key.0);
    }
    runtime.checkpoint(None)?;
    Ok(OverlapCellKey(hasher.finalize().into()))
}

fn canonical_boundary_bytes<O: GlobalFlatFoldabilityObserver + ?Sized>(
    boundary: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<Vec<u8>>> {
    let mut encoded = Vec::with_capacity(boundary.len());
    let mut control_poll = 0_usize;
    for point in boundary {
        runtime.poll_control(&mut control_poll)?;
        let mut bytes = rational_bytes(&point.x)?;
        bytes.extend_from_slice(&rational_bytes(&point.y)?);
        encoded.push(bytes);
    }
    let Some((mut start, first)) = encoded.iter().enumerate().next() else {
        return Err(FacewiseAbort::Execution(internal_error()));
    };
    let mut minimum = first;
    for (index, candidate) in encoded.iter().enumerate().skip(1) {
        runtime.poll_control(&mut control_poll)?;
        if candidate < minimum {
            start = index;
            minimum = candidate;
        }
    }
    let len = encoded.len();
    let mut forward_vs_reverse = Ordering::Equal;
    for offset in 0..len {
        runtime.poll_control(&mut control_poll)?;
        let ordering = encoded[(start + offset) % len].cmp(&encoded[(start + len - offset) % len]);
        if ordering != Ordering::Equal {
            forward_vs_reverse = ordering;
            break;
        }
    }
    let reverse = forward_vs_reverse == Ordering::Greater;
    let mut canonical = Vec::with_capacity(len);
    for offset in 0..len {
        runtime.poll_control(&mut control_poll)?;
        let index = if reverse {
            (start + len - offset) % len
        } else {
            (start + offset) % len
        };
        canonical.push(std::mem::take(&mut encoded[index]));
    }
    runtime.checkpoint(None)?;
    Ok(canonical)
}

fn internal_error() -> GlobalFlatFoldabilityExecutionError {
    GlobalFlatFoldabilityExecutionError::Internal {
        reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
    }
}

fn next_vector_capacity<O: GlobalFlatFoldabilityObserver + ?Sized>(
    current_capacity: usize,
    current_len: usize,
    maximum_len: usize,
    runtime: &Runtime<'_, O>,
) -> FacewiseResult<usize> {
    let required = current_len
        .checked_add(1)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    if required > maximum_len {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let geometric = if current_capacity == 0 {
        4
    } else {
        current_capacity.saturating_mul(2)
    };
    let next = geometric.max(required).min(maximum_len);
    if next < required || next == usize::MAX {
        return Err(runtime.exact_storage_limit_failure(usize::MAX));
    }
    Ok(next)
}

fn layer_order_derivation(
    embedding: &FlatEmbedding,
    reference_face: LayerFace,
    overlap_cell_count: usize,
    constraint_count: usize,
) -> LayerOrderDerivation {
    if embedding.hinges.is_empty() && embedding.faces.len() == 1 {
        LayerOrderDerivation::SingleFace {
            face: reference_face,
        }
    } else if embedding.hinges.len() == 1 && embedding.faces.len() == 2 {
        let hinge = &embedding.hinges[0];
        LayerOrderDerivation::SingleHinge {
            hinge_edge: hinge.edge,
            assignment: hinge.assignment,
            canonical_first: embedding.faces[hinge.first_face].source.layer,
            canonical_second: embedding.faces[hinge.second_face].source.layer,
        }
    } else {
        LayerOrderDerivation::FacewiseCertificate {
            reference_face,
            overlap_cell_count,
            constraint_count,
        }
    }
}

fn trusted_required_face_index(embedding: &FlatEmbedding, required: LayerFace) -> Option<usize> {
    let required_key = (required.face_key, required.face_id.canonical_bytes());
    let index = embedding
        .faces
        .binary_search_by(|candidate| {
            let candidate = candidate.source.layer;
            (candidate.face_key, candidate.face_id.canonical_bytes()).cmp(&required_key)
        })
        .ok()?;
    (embedding.faces[index].source.layer == required).then_some(index)
}

fn required_face_registry_is_strictly_canonical(embedding: &FlatEmbedding) -> bool {
    embedding.faces.windows(2).all(|faces| {
        let first = faces[0].source.layer;
        let second = faces[1].source.layer;
        (first.face_key, first.face_id.canonical_bytes())
            < (second.face_key, second.face_id.canonical_bytes())
    })
}

fn required_pair_direction(
    embedding: &FlatEmbedding,
    variable: (usize, usize),
    canonical_second_above_first: bool,
) -> (ori_domain::FaceId, ori_domain::FaceId) {
    let (lower, upper) = if canonical_second_above_first {
        variable
    } else {
        (variable.1, variable.0)
    };
    (
        embedding.faces[lower].source.layer.face_id,
        embedding.faces[upper].source.layer.face_id,
    )
}

struct RequiredPairOverlay {
    fixed_assignments: Vec<Option<bool>>,
    required_assignments: Vec<(usize, bool)>,
    storage_bytes: usize,
}

impl RequiredPairOverlay {
    fn release<O: GlobalFlatFoldabilityObserver + ?Sized>(
        self,
        runtime: &mut Runtime<'_, O>,
    ) -> FacewiseResult<()> {
        let storage_bytes = self.storage_bytes;
        drop(self);
        runtime.release_constraint_storage(storage_bytes)
    }
}

fn overlay_required_pair_orders<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    problem: &ConstraintProblem,
    required_pair_orders: &[RequiredLayerOrderPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<RequiredPairOverlay> {
    if !required_face_registry_is_strictly_canonical(embedding) {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let required_bytes = runtime.allocation_bytes(
        required_pair_orders.len(),
        std::mem::size_of::<(usize, bool)>(),
    )?;
    runtime.add_constraint_storage(required_bytes)?;
    let mut required_assignments = Vec::new();
    required_assignments
        .try_reserve_exact(required_pair_orders.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for required in required_pair_orders {
        runtime.checkpoint(None)?;
        let Some(lower) = trusted_required_face_index(embedding, required.lower_face) else {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::UnknownFace {
                    face: required.lower_face.face_id,
                },
            ));
        };
        let Some(upper) = trusted_required_face_index(embedding, required.upper_face) else {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::UnknownFace {
                    face: required.upper_face.face_id,
                },
            ));
        };
        if lower == upper {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::EqualFace {
                    face: required.lower_face.face_id,
                },
            ));
        }
        let pair = ordered_pair(lower, upper);
        let Ok(variable) = problem.variables.binary_search(&pair) else {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::NonOverlappingPair {
                    lower: required.lower_face.face_id,
                    upper: required.upper_face.face_id,
                },
            ));
        };
        required_assignments.push((variable, upper == pair.1));
    }
    required_assignments.sort_unstable();
    let mut group_start = 0;
    while group_start < required_assignments.len() {
        let variable_index = required_assignments[group_start].0;
        let mut group_end = group_start + 1;
        while group_end < required_assignments.len()
            && required_assignments[group_end].0 == variable_index
        {
            group_end += 1;
        }
        let group = &required_assignments[group_start..group_end];
        let variable = problem.variables[variable_index];
        let has_false = group.iter().any(|(_, direction)| !*direction);
        let has_true = group.iter().any(|(_, direction)| *direction);
        if has_false && has_true {
            let (first, second) = required_pair_direction(embedding, variable, false);
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::ConflictingPair { first, second },
            ));
        }
        if group.len() > 1 {
            let (lower, upper) = required_pair_direction(embedding, variable, group[0].1);
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::DuplicatePair { lower, upper },
            ));
        }
        group_start = group_end;
    }

    let fixed_bytes = runtime.allocation_bytes(
        problem.fixed_assignments.len(),
        std::mem::size_of::<Option<bool>>(),
    )?;
    runtime.add_constraint_storage(fixed_bytes)?;
    let mut fixed_assignments = Vec::new();
    fixed_assignments
        .try_reserve_exact(problem.fixed_assignments.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    fixed_assignments.extend_from_slice(&problem.fixed_assignments);
    for &(variable, required) in &required_assignments {
        if fixed_assignments[variable].is_some_and(|fixed| fixed != required) {
            let (lower, upper) =
                required_pair_direction(embedding, problem.variables[variable], required);
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::ContradictsTrustedFixedOrder { lower, upper },
            ));
        }
        fixed_assignments[variable] = Some(required);
    }
    let storage_bytes = required_bytes
        .checked_add(fixed_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    Ok(RequiredPairOverlay {
        fixed_assignments,
        required_assignments,
        storage_bytes,
    })
}

fn solve_layer_order<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: FlatEmbedding,
    pairs: Vec<OverlapPair>,
    cells: Vec<OverlapCell>,
    provenance: GlobalFlatFoldabilityProvenance,
    required_pair_orders: Option<&[RequiredLayerOrderPair]>,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<SolveSuccess> {
    runtime.advance(GlobalFlatFoldabilityPhase::BuildingConstraints, None)?;
    let problem = build_constraint_problem(&embedding, &pairs, &cells, runtime, true)?;
    if problem.variables.len() != runtime.work.overlap_face_pairs
        || problem.constraints.len() != runtime.work.constraints
    {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let required_overlay = required_pair_orders
        .map(|required| overlay_required_pair_orders(&embedding, &problem, required, runtime))
        .transpose()?;
    let solver_fixed_assignments = required_overlay
        .as_ref()
        .map_or(problem.fixed_assignments.as_slice(), |overlay| {
            overlay.fixed_assignments.as_slice()
        });
    runtime.advance(GlobalFlatFoldabilityPhase::Propagating, None)?;
    let solver_memory_limit = runtime.remaining_storage_bytes()?;
    let solver_result = solve_constraints_with_memory(
        problem.variables.len(),
        &problem.constraints,
        solver_fixed_assignments,
        runtime.limits.max_search_nodes,
        solver_memory_limit,
        |event, search_nodes| runtime.constraint_solver_control(event, search_nodes),
    );
    let assignment = match solver_result {
        ConstraintSolverResult::Satisfied {
            assignment,
            search_nodes,
        } => {
            runtime.set_search_nodes(search_nodes)?;
            runtime.add_constraint_storage(
                runtime.allocation_bytes(assignment.len(), std::mem::size_of::<bool>())?,
            )?;
            assignment
        }
        ConstraintSolverResult::Unsatisfied {
            conflict_constraint,
            search_nodes,
        } => {
            runtime.set_search_nodes(search_nodes)?;
            if required_pair_orders.is_some() {
                return Err(FacewiseAbort::RequiredLayerOrder(
                    RequiredLayerOrderError::Unsatisfied,
                ));
            }
            if let Some(conflict) = conflict_constraint {
                if conflict.logical_index >= problem.constraints.len() {
                    return Err(FacewiseAbort::Execution(internal_error()));
                }
                return Err(constraint_conflict_contradiction(conflict, &embedding));
            }
            return Err(FacewiseAbort::Impossible(
                GlobalFlatFoldabilityImpossibleReason::FacewiseSearchExhausted {
                    variable_count: problem.variables.len(),
                    constraint_count: problem.constraints.len(),
                },
            ));
        }
        ConstraintSolverResult::SearchNodeLimit { observed } => {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::SearchNodes,
                    limit: runtime.limits.max_search_nodes,
                    observed,
                },
            ));
        }
        ConstraintSolverResult::DeadlineReached { search_nodes } => {
            runtime.set_search_nodes(search_nodes)?;
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: runtime.phase,
                },
            ));
        }
        ConstraintSolverResult::Cancelled => {
            return Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled,
            ));
        }
        ConstraintSolverResult::WorkingMemoryLimit { observed } => {
            let used = runtime
                .exact_storage
                .total()
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            let total_observed = used.saturating_add(observed);
            return Err(runtime.exact_storage_limit_failure(total_observed));
        }
        ConstraintSolverResult::InvalidConstraint => {
            return Err(FacewiseAbort::Execution(internal_error()));
        }
    };
    if runtime.phase < GlobalFlatFoldabilityPhase::VerifyingCertificate {
        runtime.advance(
            GlobalFlatFoldabilityPhase::VerifyingCertificate,
            Some(problem.constraints.len()),
        )?;
    }
    verify_facewise_certificate(&embedding, &pairs, &cells, &problem, &assignment, runtime)?;
    if required_overlay.as_ref().is_some_and(|overlay| {
        overlay
            .required_assignments
            .iter()
            .any(|(variable, expected)| assignment.get(*variable) != Some(expected))
    }) {
        return Err(FacewiseAbort::RequiredLayerOrder(
            RequiredLayerOrderError::CertificateReverificationFailed,
        ));
    }
    if let Some(overlay) = required_overlay {
        overlay.release(runtime)?;
    }
    runtime.add_certificate_structure_storage(runtime.allocation_bytes(
        problem.variables.len(),
        std::mem::size_of::<((usize, usize), bool)>(),
    )?)?;
    let pair_values = PairValues::try_from_parallel(&problem.variables, &assignment)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    drop(assignment);
    drop(problem);
    runtime.clear_constraint_storage();

    let reference_face = embedding.faces[embedding.reference_face].source.layer;
    let face_pair_order_bytes = runtime.allocation_bytes(
        pair_values.len(),
        std::mem::size_of::<FacePairOrderSnapshot>(),
    )?;
    runtime.add_certificate_structure_storage(face_pair_order_bytes)?;
    let mut face_pair_orders = Vec::new();
    face_pair_orders
        .try_reserve_exact(pair_values.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for &((first, second), second_above_first) in pair_values.iter() {
        let (lower, upper) = if second_above_first {
            (first, second)
        } else {
            (second, first)
        };
        let mut supporting_cells = Vec::new();
        let mut supporting_cell_poll = 0_usize;
        for cell in &cells {
            runtime.poll_control(&mut supporting_cell_poll)?;
            if cell.covering_faces.contains(&first) && cell.covering_faces.contains(&second) {
                if supporting_cells.len() == supporting_cells.capacity() {
                    let prior_capacity = supporting_cells.capacity();
                    let next_capacity = next_vector_capacity(
                        prior_capacity,
                        supporting_cells.len(),
                        cells.len(),
                        runtime,
                    )?;
                    let next_bytes = runtime
                        .allocation_bytes(next_capacity, std::mem::size_of::<OverlapCellKey>())?;
                    runtime.ensure_transient_exact_storage(next_bytes)?;
                    runtime.add_certificate_structure_storage(runtime.allocation_bytes(
                        next_capacity - prior_capacity,
                        std::mem::size_of::<OverlapCellKey>(),
                    )?)?;
                    supporting_cells
                        .try_reserve_exact(next_capacity - supporting_cells.len())
                        .map_err(|_| {
                            runtime
                                .exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
                        })?;
                }
                supporting_cells.push(cell.key);
            }
        }
        runtime.checkpoint(None)?;
        supporting_cells.sort_unstable_by_key(|key| key.0);
        face_pair_orders.push(FacePairOrderSnapshot {
            lower_face: embedding.faces[lower].source.layer,
            upper_face: embedding.faces[upper].source.layer,
            supporting_cells,
        });
    }
    face_pair_orders.sort_unstable_by_key(|order| {
        (
            order.lower_face.face_key,
            order.upper_face.face_key,
            order.lower_face.face_id.canonical_bytes(),
            order.upper_face.face_id.canonical_bytes(),
        )
    });

    let overlap_cell_snapshot_bytes =
        runtime.allocation_bytes(cells.len(), std::mem::size_of::<OverlapCellSnapshot>())?;
    runtime.add_certificate_structure_storage(overlap_cell_snapshot_bytes)?;
    let mut overlap_cells = Vec::new();
    overlap_cells
        .try_reserve_exact(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut maximum_ply = 1_usize;
    for cell in &cells {
        runtime.checkpoint(None)?;
        let boundary_structure_bytes = runtime.allocation_bytes(
            cell.boundary.len(),
            std::mem::size_of::<crate::ExactPointValue>(),
        )?;
        let covering_structure_bytes = runtime
            .allocation_bytes(cell.covering_faces.len(), std::mem::size_of::<LayerFace>())?;
        let ordered_structure_bytes = runtime.allocation_bytes(
            cell.covering_faces.len(),
            std::mem::size_of::<ori_domain::FaceId>(),
        )?;
        let inner_structure_bytes = boundary_structure_bytes
            .checked_add(covering_structure_bytes)
            .and_then(|total| total.checked_add(ordered_structure_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        runtime.add_certificate_structure_storage(inner_structure_bytes)?;
        let ordered = order_cell_faces(&cell.covering_faces, &pair_values, runtime)?;
        runtime.ensure_transient_exact_storage(
            runtime.allocation_bytes(ordered.capacity(), std::mem::size_of::<usize>())?,
        )?;
        maximum_ply = maximum_ply.max(ordered.len());
        runtime.add_snapshot_exact_storage(exact_storage_bytes_points(&cell.boundary)?)?;
        let mut exact_boundary = Vec::new();
        exact_boundary
            .try_reserve_exact(cell.boundary.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        for point in &cell.boundary {
            exact_boundary.push(point.to_value());
        }
        let mut covering_face_snapshots = Vec::new();
        covering_face_snapshots
            .try_reserve_exact(cell.covering_faces.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        for index in &cell.covering_faces {
            covering_face_snapshots.push(embedding.faces[*index].source.layer);
        }
        let mut bottom_to_top_faces = Vec::new();
        bottom_to_top_faces
            .try_reserve_exact(ordered.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        for index in &ordered {
            bottom_to_top_faces.push(embedding.faces[*index].source.layer.face_id);
        }
        overlap_cells.push(OverlapCellSnapshot {
            cell_key: cell.key,
            exact_boundary,
            covering_faces: covering_face_snapshots,
            bottom_to_top_faces,
        });
    }
    overlap_cells.sort_unstable_by_key(|cell| cell.cell_key.0);
    let global_order =
        canonical_global_linear_extension(embedding.faces.len(), &pair_values, runtime)?;
    let global_bottom_to_top = if let Some(order) = global_order {
        let final_bytes =
            runtime.allocation_bytes(order.len(), std::mem::size_of::<LayerFace>())?;
        let order_bytes =
            runtime.allocation_bytes(order.capacity(), std::mem::size_of::<usize>())?;
        runtime.ensure_transient_exact_storage(
            final_bytes
                .checked_add(order_bytes)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        runtime.add_certificate_structure_storage(final_bytes)?;
        let mut faces = Vec::new();
        faces.try_reserve_exact(order.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        for index in order {
            faces.push(embedding.faces[index].source.layer);
        }
        Some(faces)
    } else {
        None
    };
    runtime.add_certificate_structure_storage(
        runtime.allocation_bytes(embedding.faces.len(), std::mem::size_of::<LayerFace>())?,
    )?;
    let mut material_faces = Vec::new();
    material_faces
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for face in &embedding.faces {
        material_faces.push(face.source.layer);
    }
    runtime.add_certificate_structure_storage(runtime.allocation_bytes(
        embedding.faces.len(),
        std::mem::size_of::<FoldedFaceSnapshot>(),
    )?)?;
    let mut folded_faces = Vec::new();
    folded_faces
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for face in &embedding.faces {
        runtime.add_snapshot_exact_storage(exact_storage_bytes_transform(&face.transform)?)?;
        folded_faces.push(FoldedFaceSnapshot {
            face: face.source.layer,
            source_to_flat: face.transform.to_value(),
            orientation: if face.front_up {
                FoldedFaceOrientation::FrontUp
            } else {
                FoldedFaceOrientation::BackUp
            },
        });
    }
    let proof_summary = FacewiseProofSummary {
        material_faces: embedding.faces.len(),
        overlap_face_pairs: pair_values.len(),
        overlap_cells: overlap_cells.len(),
        constraints: runtime.work.constraints,
        search_nodes: runtime.work.search_nodes,
        maximum_ply,
        certificate_bytes: 0,
    };
    let derivation = layer_order_derivation(
        &embedding,
        reference_face,
        overlap_cells.len(),
        runtime.work.constraints,
    );
    let mut layer_order = LayerOrderSnapshot {
        model_id: LayerOrderModelId::FacewiseLayerOrderV1,
        material_faces,
        global_bottom_to_top,
        provenance: LayerOrderProvenance {
            source: provenance,
            derivation,
        },
        reference_face: Some(reference_face),
        folded_faces,
        overlap_cells,
        face_pair_orders,
        proof_summary: Some(proof_summary),
    };
    finalize_certificate_size(&mut layer_order, runtime)?;
    verify_layer_order_snapshot(
        &layer_order,
        &embedding,
        &cells,
        &pair_values,
        provenance,
        runtime,
    )?;
    if let Some(required_pair_orders) = required_pair_orders {
        verify_required_pair_orders_against_snapshot(&layer_order, required_pair_orders, runtime)?;
    }
    Ok(SolveSuccess {
        reason: GlobalFlatFoldabilityPossibleReason::FacewiseConstraintCertificate {
            reference_face,
            overlap_cell_count: layer_order.overlap_cells.len(),
            constraint_count: runtime.work.constraints,
        },
        layer_order,
    })
}

fn verify_required_pair_orders_against_snapshot<O: GlobalFlatFoldabilityObserver + ?Sized>(
    snapshot: &LayerOrderSnapshot,
    required_pair_orders: &[RequiredLayerOrderPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let canonical_key = |face: LayerFace| (face.face_key, face.face_id.canonical_bytes());
    if snapshot
        .material_faces
        .windows(2)
        .any(|faces| canonical_key(faces[0]) >= canonical_key(faces[1]))
    {
        return Err(FacewiseAbort::RequiredLayerOrder(
            RequiredLayerOrderError::CertificateReverificationFailed,
        ));
    }
    let face_is_material = |required: LayerFace| {
        snapshot
            .material_faces
            .binary_search_by_key(&canonical_key(required), |face| canonical_key(*face))
            .ok()
            .is_some_and(|index| snapshot.material_faces[index] == required)
    };
    for order in &snapshot.face_pair_orders {
        runtime.checkpoint(None)?;
        if !face_is_material(order.lower_face)
            || !face_is_material(order.upper_face)
            || order.lower_face.face_id == order.upper_face.face_id
        {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::CertificateReverificationFailed,
            ));
        }
    }
    let pair_key = |lower: LayerFace, upper: LayerFace| {
        (
            lower.face_key,
            upper.face_key,
            lower.face_id.canonical_bytes(),
            upper.face_id.canonical_bytes(),
        )
    };
    for required in required_pair_orders {
        runtime.checkpoint(None)?;
        if !face_is_material(required.lower_face)
            || !face_is_material(required.upper_face)
            || required.lower_face.face_id == required.upper_face.face_id
        {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::CertificateReverificationFailed,
            ));
        }
        let key = pair_key(required.lower_face, required.upper_face);
        let Some(order) = snapshot
            .face_pair_orders
            .binary_search_by_key(&key, |order| pair_key(order.lower_face, order.upper_face))
            .ok()
            .and_then(|index| snapshot.face_pair_orders.get(index))
        else {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::CertificateReverificationFailed,
            ));
        };
        if order.lower_face != required.lower_face || order.upper_face != required.upper_face {
            return Err(FacewiseAbort::RequiredLayerOrder(
                RequiredLayerOrderError::CertificateReverificationFailed,
            ));
        }
    }
    Ok(())
}

fn verify_layer_order_snapshot<O: GlobalFlatFoldabilityObserver + ?Sized>(
    layer_order: &LayerOrderSnapshot,
    embedding: &FlatEmbedding,
    cells: &[OverlapCell],
    pair_values: &PairValues,
    provenance: GlobalFlatFoldabilityProvenance,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    runtime.clear_verification_storage();
    runtime.checkpoint(None)?;
    verify_canonical_overlap_cells(&embedding.faces, cells, runtime)?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(embedding.faces.len(), std::mem::size_of::<LayerFace>())?,
    )?;
    let mut expected_material_faces = Vec::new();
    expected_material_faces
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for face in &embedding.faces {
        expected_material_faces.push(face.source.layer);
    }
    let reference_face = embedding.faces[embedding.reference_face].source.layer;
    let expected_derivation = layer_order_derivation(
        embedding,
        reference_face,
        cells.len(),
        runtime.work.constraints,
    );
    if layer_order.model_id != LayerOrderModelId::FacewiseLayerOrderV1
        || layer_order.provenance.source != provenance
        || layer_order.provenance.derivation != expected_derivation
        || layer_order.reference_face != Some(reference_face)
        || layer_order.material_faces != expected_material_faces
        || layer_order.folded_faces.len() != embedding.faces.len()
        || layer_order.overlap_cells.len() != cells.len()
        || layer_order.face_pair_orders.len() != pair_values.len()
    {
        return Err(certificate_failure());
    }
    for (snapshot, face) in layer_order.folded_faces.iter().zip(&embedding.faces) {
        runtime.ensure_transient_exact_storage(exact_storage_bytes_transform(&face.transform)?)?;
        if snapshot.face != face.source.layer
            || snapshot.source_to_flat != face.transform.to_value()
            || snapshot.orientation
                != if face.front_up {
                    FoldedFaceOrientation::FrontUp
                } else {
                    FoldedFaceOrientation::BackUp
                }
        {
            return Err(certificate_failure());
        }
    }

    for cell in cells {
        runtime.checkpoint(None)?;
        if overlap_cell_key(
            &cell.boundary,
            &cell.covering_faces,
            &embedding.faces,
            runtime,
        )? != cell.key
        {
            return Err(certificate_failure());
        }
    }
    runtime.add_verification_storage(
        runtime.allocation_bytes(cells.len(), std::mem::size_of::<OverlapCellKey>())?,
    )?;
    let mut internal_cell_keys = Vec::new();
    internal_cell_keys
        .try_reserve_exact(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in cells {
        internal_cell_keys.push(cell.key);
    }
    internal_cell_keys.sort_unstable_by_key(|key| key.0);
    if internal_cell_keys.windows(2).any(|keys| keys[0] == keys[1]) {
        return Err(certificate_failure());
    }
    runtime.add_verification_storage(runtime.allocation_bytes(
        layer_order.overlap_cells.len(),
        std::mem::size_of::<OverlapCellKey>(),
    )?)?;
    let mut snapshot_cell_keys = Vec::new();
    snapshot_cell_keys
        .try_reserve_exact(layer_order.overlap_cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in &layer_order.overlap_cells {
        snapshot_cell_keys.push(cell.cell_key);
    }
    if snapshot_cell_keys != internal_cell_keys {
        return Err(certificate_failure());
    }
    let internal_cell_entry_bytes = std::mem::size_of::<(OverlapCellKey, &OverlapCell)>()
        .checked_add(3 * std::mem::size_of::<usize>())
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(cells.len(), internal_cell_entry_bytes)?,
    )?;
    let mut internal_cells = HashMap::new();
    internal_cells
        .try_reserve(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in cells {
        internal_cells.insert(cell.key, cell);
    }
    for snapshot in &layer_order.overlap_cells {
        runtime.checkpoint(None)?;
        let Some(cell) = internal_cells.get(&snapshot.cell_key).copied() else {
            return Err(certificate_failure());
        };
        let expected_order = order_cell_faces(&cell.covering_faces, pair_values, runtime)?;
        let expected_boundary_structure = runtime.allocation_bytes(
            cell.boundary.len(),
            std::mem::size_of::<crate::ExactPointValue>(),
        )?;
        let expected_covering_structure = runtime
            .allocation_bytes(cell.covering_faces.len(), std::mem::size_of::<LayerFace>())?;
        let expected_order_structure = runtime.allocation_bytes(
            expected_order.len(),
            std::mem::size_of::<ori_domain::FaceId>(),
        )?;
        let expected_structure = expected_boundary_structure
            .checked_add(expected_covering_structure)
            .and_then(|total| total.checked_add(expected_order_structure))
            .and_then(|total| {
                total.checked_add(
                    runtime
                        .allocation_bytes(expected_order.capacity(), std::mem::size_of::<usize>())
                        .ok()?,
                )
            })
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        let expected_exact = exact_storage_bytes_points(&cell.boundary)?;
        runtime.ensure_transient_exact_storage(
            expected_structure
                .checked_add(expected_exact)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        if snapshot.exact_boundary
            != cell
                .boundary
                .iter()
                .map(Point::to_value)
                .collect::<Vec<_>>()
            || snapshot.covering_faces
                != cell
                    .covering_faces
                    .iter()
                    .map(|index| embedding.faces[*index].source.layer)
                    .collect::<Vec<_>>()
            || snapshot.bottom_to_top_faces
                != expected_order
                    .iter()
                    .map(|index| embedding.faces[*index].source.layer.face_id)
                    .collect::<Vec<_>>()
        {
            return Err(certificate_failure());
        }
    }

    runtime.add_verification_storage(runtime.allocation_bytes(
        pair_values.len(),
        std::mem::size_of::<FacePairOrderSnapshot>(),
    )?)?;
    let mut expected_face_pair_orders = Vec::new();
    expected_face_pair_orders
        .try_reserve_exact(pair_values.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for &((first, second), canonical_second_above_first) in pair_values.iter() {
        runtime.checkpoint(None)?;
        let (lower, upper) = if canonical_second_above_first {
            (first, second)
        } else {
            (second, first)
        };
        let mut supporting_cells = Vec::new();
        let mut supporting_cell_poll = 0_usize;
        for cell in cells {
            runtime.poll_control(&mut supporting_cell_poll)?;
            if cell.covering_faces.contains(&first) && cell.covering_faces.contains(&second) {
                if supporting_cells.len() == supporting_cells.capacity() {
                    let prior_capacity = supporting_cells.capacity();
                    let next_capacity = next_vector_capacity(
                        prior_capacity,
                        supporting_cells.len(),
                        cells.len(),
                        runtime,
                    )?;
                    let next_bytes = runtime
                        .allocation_bytes(next_capacity, std::mem::size_of::<OverlapCellKey>())?;
                    runtime.ensure_transient_exact_storage(next_bytes)?;
                    runtime.add_verification_storage(runtime.allocation_bytes(
                        next_capacity - prior_capacity,
                        std::mem::size_of::<OverlapCellKey>(),
                    )?)?;
                    supporting_cells
                        .try_reserve_exact(next_capacity - supporting_cells.len())
                        .map_err(|_| {
                            runtime
                                .exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
                        })?;
                }
                supporting_cells.push(cell.key);
            }
        }
        runtime.checkpoint(None)?;
        supporting_cells.sort_unstable_by_key(|key| key.0);
        if supporting_cells.is_empty() {
            return Err(certificate_failure());
        }
        expected_face_pair_orders.push(FacePairOrderSnapshot {
            lower_face: embedding.faces[lower].source.layer,
            upper_face: embedding.faces[upper].source.layer,
            supporting_cells,
        });
    }
    expected_face_pair_orders.sort_unstable_by_key(|order| {
        (
            order.lower_face.face_key,
            order.upper_face.face_key,
            order.lower_face.face_id.canonical_bytes(),
            order.upper_face.face_id.canonical_bytes(),
        )
    });
    if layer_order.face_pair_orders != expected_face_pair_orders {
        return Err(certificate_failure());
    }

    let recomputed_order =
        canonical_global_linear_extension(embedding.faces.len(), pair_values, runtime)?;
    let recomputed_global = if let Some(order) = recomputed_order {
        let mapped_bytes =
            runtime.allocation_bytes(order.len(), std::mem::size_of::<LayerFace>())?;
        let order_bytes =
            runtime.allocation_bytes(order.capacity(), std::mem::size_of::<usize>())?;
        runtime.add_verification_storage(mapped_bytes)?;
        runtime.ensure_transient_exact_storage(
            order_bytes
                .checked_add(mapped_bytes)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        let mut mapped = Vec::new();
        mapped.try_reserve_exact(order.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        for index in order {
            mapped.push(embedding.faces[index].source.layer);
        }
        Some(mapped)
    } else {
        None
    };
    if layer_order.global_bottom_to_top != recomputed_global {
        return Err(certificate_failure());
    }
    let maximum_ply = cells
        .iter()
        .map(|cell| cell.covering_faces.len())
        .max()
        .unwrap_or(1);
    let Some(summary) = layer_order.proof_summary else {
        return Err(certificate_failure());
    };
    if summary.material_faces != embedding.faces.len()
        || summary.overlap_face_pairs != pair_values.len()
        || summary.overlap_cells != cells.len()
        || summary.constraints != runtime.work.constraints
        || summary.search_nodes != runtime.work.search_nodes
        || summary.maximum_ply != maximum_ply
    {
        return Err(certificate_failure());
    }
    let serialized_bytes = serialized_certificate_size(layer_order, runtime)?;
    if summary.certificate_bytes != runtime.work.certificate_bytes
        || summary.certificate_bytes != serialized_bytes
    {
        return Err(certificate_failure());
    }
    drop(recomputed_global);
    drop(expected_face_pair_orders);
    drop(internal_cells);
    drop(snapshot_cell_keys);
    drop(internal_cell_keys);
    drop(expected_material_faces);
    runtime.clear_verification_storage();
    runtime.checkpoint(None)?;
    Ok(())
}

#[derive(Clone, PartialEq, Eq)]
struct ConstraintProblem {
    variables: Vec<(usize, usize)>,
    constraints: ConstraintSet,
    fixed_assignments: Vec<Option<bool>>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct PairValues(Vec<((usize, usize), bool)>);

impl PairValues {
    fn try_from_parallel(
        variables: &[(usize, usize)],
        assignment: &[bool],
    ) -> Result<Self, std::collections::TryReserveError> {
        let mut values = Vec::new();
        values.try_reserve_exact(variables.len())?;
        values.extend(variables.iter().copied().zip(assignment.iter().copied()));
        Ok(Self(values))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn get(&self, key: &(usize, usize)) -> Option<&bool> {
        self.0
            .binary_search_by_key(key, |(pair, _)| *pair)
            .ok()
            .map(|index| &self.0[index].1)
    }

    fn contains_key(&self, key: &(usize, usize)) -> bool {
        self.get(key).is_some()
    }

    fn keys(&self) -> impl ExactSizeIterator<Item = &(usize, usize)> {
        self.0.iter().map(|(pair, _)| pair)
    }

    fn iter(&self) -> impl ExactSizeIterator<Item = &((usize, usize), bool)> {
        self.0.iter()
    }

    #[cfg(test)]
    fn insert(&mut self, key: (usize, usize), value: bool) {
        match self.0.binary_search_by_key(&key, |(pair, _)| *pair) {
            Ok(index) => self.0[index].1 = value,
            Err(index) => self.0.insert(index, (key, value)),
        }
    }
}

#[derive(Clone, Copy)]
enum ConstraintStorageScope {
    Primary,
    Verification,
}

fn add_constraint_problem_storage<O: GlobalFlatFoldabilityObserver + ?Sized>(
    runtime: &mut Runtime<'_, O>,
    scope: ConstraintStorageScope,
    additional: usize,
) -> FacewiseResult<()> {
    match scope {
        ConstraintStorageScope::Primary => runtime.add_constraint_storage(additional),
        ConstraintStorageScope::Verification => runtime.add_verification_storage(additional),
    }
}

fn ensure_constraint_scope_transient<O: GlobalFlatFoldabilityObserver + ?Sized>(
    runtime: &Runtime<'_, O>,
    scope: ConstraintStorageScope,
    additional: usize,
) -> FacewiseResult<()> {
    match scope {
        ConstraintStorageScope::Primary => runtime.ensure_constraint_transient_storage(additional),
        ConstraintStorageScope::Verification => runtime.ensure_transient_exact_storage(additional),
    }
}

fn ensure_constraint_construction_headroom<O: GlobalFlatFoldabilityObserver + ?Sized>(
    runtime: &Runtime<'_, O>,
    scope: ConstraintStorageScope,
) -> FacewiseResult<()> {
    let maximum_inner_bytes = 6_usize
        .checked_mul(std::mem::size_of::<usize>())
        .and_then(|total| total.checked_add(64 * std::mem::size_of::<u8>()))
        .and_then(|total| total.checked_add(4 * std::mem::size_of::<usize>()))
        .and_then(|total| total.checked_add(std::mem::size_of::<TupleConstraint>()))
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    ensure_constraint_scope_transient(runtime, scope, maximum_inner_bytes)
}

fn build_constraint_problem<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    pairs: &[OverlapPair],
    cells: &[OverlapCell],
    runtime: &mut Runtime<'_, O>,
    record_work: bool,
) -> FacewiseResult<ConstraintProblem> {
    let storage_scope = if record_work {
        ConstraintStorageScope::Primary
    } else {
        ConstraintStorageScope::Verification
    };
    runtime.checkpoint(None)?;
    let variable_bytes =
        runtime.allocation_bytes(pairs.len(), std::mem::size_of::<(usize, usize)>())?;
    add_constraint_problem_storage(runtime, storage_scope, variable_bytes)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(pairs.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut variable_poll = 0_usize;
    for pair in pairs {
        runtime.poll_control(&mut variable_poll)?;
        variables.push(ordered_pair(pair.first, pair.second));
    }
    variables.sort_unstable();
    let original_variable_count = variables.len();
    variables.dedup();
    if variables.len() != original_variable_count {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
            },
        ));
    }
    let mut constraints = Vec::new();
    let mut transitivity_families = Vec::new();
    let mut transitivity_constraint_count = 0_usize;
    let fixed_assignment_bytes =
        runtime.allocation_bytes(variables.len(), std::mem::size_of::<Option<bool>>())?;
    add_constraint_problem_storage(runtime, storage_scope, fixed_assignment_bytes)?;
    let mut fixed_assignments = Vec::new();
    fixed_assignments
        .try_reserve_exact(variables.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    fixed_assignments.resize(variables.len(), None);

    for (variable, &(first, second)) in variables.iter().enumerate() {
        runtime.checkpoint(None)?;
        ensure_constraint_construction_headroom(runtime, storage_scope)?;
        push_constraint(
            &mut constraints,
            TupleConstraint {
                kind: FacewiseConstraintKind::Antisymmetry,
                variables: vec![variable],
                allowed_rows: vec![0, 1],
                faces: vec![first, second],
                supporting_cell: supporting_cell(cells, &[first, second], runtime)?,
            },
            runtime,
            storage_scope,
            0,
        )?;
    }

    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        let key = ordered_pair(hinge.first_face, hinge.second_face);
        let Ok(variable) = variables.binary_search(&key) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
                },
            ));
        };
        let canonical_value = mountain_valley_canonical_value(
            hinge.assignment,
            embedding.faces[hinge.first_face].front_up,
            hinge.first_face,
            hinge.second_face,
        );
        if fixed_assignments[variable].is_some_and(|existing| existing != canonical_value) {
            ensure_constraint_construction_headroom(runtime, storage_scope)?;
            let constraint = TupleConstraint {
                kind: FacewiseConstraintKind::MountainValley,
                variables: vec![variable],
                allowed_rows: vec![u8::from(canonical_value)],
                faces: vec![key.0, key.1],
                supporting_cell: supporting_cell(cells, &[key.0, key.1], runtime)?,
            };
            return Err(constraint_contradiction(
                ConstraintView::Explicit(&constraint),
                embedding,
            ));
        }
        fixed_assignments[variable] = Some(canonical_value);
        ensure_constraint_construction_headroom(runtime, storage_scope)?;
        push_constraint(
            &mut constraints,
            TupleConstraint {
                kind: FacewiseConstraintKind::MountainValley,
                variables: vec![variable],
                allowed_rows: vec![u8::from(canonical_value)],
                faces: vec![key.0, key.1],
                supporting_cell: supporting_cell(cells, &[key.0, key.1], runtime)?,
            },
            runtime,
            storage_scope,
            0,
        )?;
    }

    for cell in cells {
        runtime.checkpoint(None)?;
        let family_count = choose_three(cell.covering_faces.len())
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        let current_logical_count = constraints
            .len()
            .checked_add(transitivity_constraint_count)
            .ok_or_else(|| constraint_limit_abort(runtime, usize::MAX))?;
        admit_constraint_batch(current_logical_count, family_count, runtime)?;
        if family_count == 0 {
            continue;
        }
        let family =
            build_transitivity_constraint_family(cell, &variables, runtime, storage_scope)?;
        push_transitivity_constraint_family(
            &mut transitivity_families,
            family,
            cells.len(),
            runtime,
            storage_scope,
        )?;
        transitivity_constraint_count = transitivity_constraint_count
            .checked_add(family_count)
            .ok_or_else(|| constraint_limit_abort(runtime, usize::MAX))?;
    }

    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        for face in 0..embedding.faces.len() {
            if face == hinge.first_face || face == hinge.second_face {
                continue;
            }
            if !segment_overlaps_face_interior(
                &hinge.first_point,
                &hinge.second_point,
                &embedding.faces[face].polygon,
                runtime,
            )? {
                continue;
            }
            let mut evidence_faces = vec![hinge.first_face, hinge.second_face, face];
            evidence_faces.sort_unstable();
            let support = supporting_cell(cells, &evidence_faces, runtime)?
                .ok_or_else(certificate_failure)?;
            let relations = [(hinge.first_face, face), (hinge.second_face, face)];
            let constraint = relation_constraint(
                RelationConstraintInput {
                    kind: FacewiseConstraintKind::TacoTortilla,
                    relations: &relations,
                    faces: &evidence_faces,
                    supporting_cell: Some(support),
                    variable_pairs: &variables,
                },
                |relations| relations[0] == relations[1],
                runtime,
                storage_scope,
            )?;
            push_constraint(
                &mut constraints,
                constraint,
                runtime,
                storage_scope,
                transitivity_constraint_count,
            )?;
        }
    }

    for first_hinge_index in 0..embedding.hinges.len() {
        runtime.checkpoint(None)?;
        for second_hinge_index in (first_hinge_index + 1)..embedding.hinges.len() {
            let first_hinge = &embedding.hinges[first_hinge_index];
            let second_hinge = &embedding.hinges[second_hinge_index];
            if !segments_overlap_in_positive_length(
                &first_hinge.first_point,
                &first_hinge.second_point,
                &second_hinge.first_point,
                &second_hinge.second_point,
                runtime,
            )? {
                continue;
            }
            let mut evidence_faces = vec![
                first_hinge.first_face,
                first_hinge.second_face,
                second_hinge.first_face,
                second_hinge.second_face,
            ];
            evidence_faces.sort_unstable();
            evidence_faces.dedup();
            if evidence_faces.len() != 4 {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                        reason:
                            FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
                    },
                ));
            }
            if !all_face_pairs_overlap(&evidence_faces, &variables) {
                continue;
            }
            let support = supporting_cell(cells, &evidence_faces, runtime)?
                .ok_or_else(certificate_failure)?;
            let relations = [
                (first_hinge.first_face, first_hinge.second_face),
                (second_hinge.first_face, second_hinge.second_face),
                (second_hinge.first_face, first_hinge.second_face),
                (first_hinge.first_face, second_hinge.second_face),
                (first_hinge.first_face, second_hinge.first_face),
                (first_hinge.second_face, second_hinge.second_face),
            ];
            let constraint = relation_constraint(
                RelationConstraintInput {
                    kind: FacewiseConstraintKind::TacoTaco,
                    relations: &relations,
                    faces: &evidence_faces,
                    supporting_cell: Some(support),
                    variable_pairs: &variables,
                },
                taco_taco_source_tuple_accepts,
                runtime,
                storage_scope,
            )?;
            push_constraint(
                &mut constraints,
                constraint,
                runtime,
                storage_scope,
                transitivity_constraint_count,
            )?;
        }
    }

    // The current target class has only M/V hinges. `Auxiliary` edges are
    // topology annotations (`AuxiliaryIgnored`), not unfolded material
    // creases, so tortilla-tortilla constraints are intentionally zero.
    constraints.sort_unstable_by(compare_constraints);
    let transitivity_insertion = constraints.partition_point(|constraint| {
        constraint_kind_rank(constraint.kind)
            < constraint_kind_rank(FacewiseConstraintKind::Transitivity)
    });
    let transitivity = TransitivityConstraints::try_new(transitivity_families, variables.len())
        .ok_or_else(internal_abort)?;
    if transitivity.len() != transitivity_constraint_count {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let constraints = ConstraintSet::new(constraints, transitivity, transitivity_insertion)
        .ok_or_else(internal_abort)?;
    if record_work {
        runtime.set_constraints(constraints.len())?;
    } else if constraints.len() > runtime.limits.max_constraints {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                limit: runtime.limits.max_constraints,
                observed: constraints.len(),
            },
        ));
    }
    Ok(ConstraintProblem {
        variables,
        constraints,
        fixed_assignments,
    })
}

fn constraint_limit_abort<O: GlobalFlatFoldabilityObserver + ?Sized>(
    runtime: &Runtime<'_, O>,
    observed: usize,
) -> FacewiseAbort {
    FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
        limit: runtime.limits.max_constraints,
        observed,
    })
}

fn admit_constraint_batch<O: GlobalFlatFoldabilityObserver + ?Sized>(
    current: usize,
    additional: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    // Preserve the old push path's control priority: cancellation/deadline is
    // observed before a logical constraint-limit failure.
    runtime.checkpoint(None)?;
    let observed = current.saturating_add(additional);
    let records_before_result = if observed > runtime.limits.max_constraints {
        runtime
            .limits
            .max_constraints
            .saturating_sub(current)
            .saturating_add(1)
            .min(additional)
    } else {
        additional
    };
    let checkpoint_count =
        records_before_result.saturating_add(CONTROL_POLL_RECORDS - 1) / CONTROL_POLL_RECORDS;
    for _ in 0..checkpoint_count {
        runtime.checkpoint(None)?;
    }
    if observed > runtime.limits.max_constraints {
        return Err(constraint_limit_abort(
            runtime,
            runtime.limits.max_constraints.saturating_add(1),
        ));
    }
    Ok(())
}

fn build_transitivity_constraint_family<O: GlobalFlatFoldabilityObserver + ?Sized>(
    cell: &OverlapCell,
    variable_pairs: &[(usize, usize)],
    runtime: &mut Runtime<'_, O>,
    storage_scope: ConstraintStorageScope,
) -> FacewiseResult<TransitivityConstraintFamily> {
    if cell.covering_faces.len() < 3
        || !cell
            .covering_faces
            .windows(2)
            .all(|faces| faces[0] < faces[1])
    {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let pair_count = choose_two(cell.covering_faces.len())
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let face_bytes =
        runtime.allocation_bytes(cell.covering_faces.len(), std::mem::size_of::<usize>())?;
    let pair_bytes = runtime.allocation_bytes(pair_count, std::mem::size_of::<usize>())?;
    let nested_bytes = face_bytes
        .checked_add(pair_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    ensure_constraint_scope_transient(runtime, storage_scope, nested_bytes)?;

    let mut covering_faces = Vec::new();
    covering_faces
        .try_reserve_exact(cell.covering_faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    covering_faces.extend_from_slice(&cell.covering_faces);
    let mut pair_variables = Vec::new();
    pair_variables
        .try_reserve_exact(pair_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut pair_poll = 0_usize;
    for first in 0..covering_faces.len() {
        for second in first + 1..covering_faces.len() {
            runtime.poll_control(&mut pair_poll)?;
            let pair = ordered_pair(covering_faces[first], covering_faces[second]);
            let variable = variable_pairs
                .binary_search(&pair)
                .map_err(|_| certificate_failure())?;
            pair_variables.push(variable);
        }
    }
    if pair_variables.len() != pair_count
        || !pair_variables
            .windows(2)
            .all(|variables| variables[0] < variables[1])
    {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    Ok(TransitivityConstraintFamily {
        covering_faces,
        pair_variables,
        supporting_cell: cell.key,
    })
}

fn push_transitivity_constraint_family<O: GlobalFlatFoldabilityObserver + ?Sized>(
    families: &mut Vec<TransitivityConstraintFamily>,
    family: TransitivityConstraintFamily,
    maximum_families: usize,
    runtime: &mut Runtime<'_, O>,
    storage_scope: ConstraintStorageScope,
) -> FacewiseResult<()> {
    runtime.checkpoint(None)?;
    let nested_bytes = runtime
        .allocation_bytes(
            family.covering_faces.capacity(),
            std::mem::size_of::<usize>(),
        )?
        .checked_add(runtime.allocation_bytes(
            family.pair_variables.capacity(),
            std::mem::size_of::<usize>(),
        )?)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let prior_capacity = families.capacity();
    let next_capacity = if families.len() == prior_capacity {
        next_vector_capacity(prior_capacity, families.len(), maximum_families, runtime)?
    } else {
        prior_capacity
    };
    let outer_bytes = runtime.allocation_bytes(
        next_capacity - prior_capacity,
        std::mem::size_of::<TransitivityConstraintFamily>(),
    )?;
    let additional = nested_bytes
        .checked_add(outer_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let old_outer_bytes = if next_capacity > prior_capacity {
        runtime.allocation_bytes(
            prior_capacity,
            std::mem::size_of::<TransitivityConstraintFamily>(),
        )?
    } else {
        0
    };
    ensure_constraint_scope_transient(
        runtime,
        storage_scope,
        additional
            .checked_add(old_outer_bytes)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    add_constraint_problem_storage(runtime, storage_scope, additional)?;
    if next_capacity > prior_capacity {
        families
            .try_reserve_exact(next_capacity - families.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
    }
    families.push(family);
    Ok(())
}

fn taco_taco_source_tuple_accepts(relations: &[bool]) -> bool {
    relations.len() == 6
        && TACO_TACO_VALID_SOURCE_TUPLES.iter().any(|tuple| {
            tuple
                .as_bytes()
                .iter()
                .zip(relations)
                .all(|(symbol, relation)| (*symbol == b'1') == *relation)
        })
}

fn mountain_valley_canonical_value(
    assignment: FoldAssignment,
    first_front_up: bool,
    first_face: usize,
    second_face: usize,
) -> bool {
    let second_above_first = (assignment == FoldAssignment::Mountain) == first_front_up;
    if first_face < second_face {
        second_above_first
    } else {
        !second_above_first
    }
}

fn push_constraint<O: GlobalFlatFoldabilityObserver + ?Sized>(
    constraints: &mut Vec<TupleConstraint>,
    constraint: TupleConstraint,
    runtime: &mut Runtime<'_, O>,
    storage_scope: ConstraintStorageScope,
    implicit_constraint_count: usize,
) -> FacewiseResult<()> {
    runtime.checkpoint(None)?;
    let observed = match constraints
        .len()
        .checked_add(implicit_constraint_count)
        .and_then(|count| count.checked_add(1))
    {
        Some(observed) => observed,
        None => {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                    limit: runtime.limits.max_constraints,
                    observed: usize::MAX,
                },
            ));
        }
    };
    if observed > runtime.limits.max_constraints {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                limit: runtime.limits.max_constraints,
                observed,
            },
        ));
    }
    let nested_bytes = runtime
        .allocation_bytes(
            constraint.variables.capacity(),
            std::mem::size_of::<usize>(),
        )?
        .checked_add(runtime.allocation_bytes(
            constraint.allowed_rows.capacity(),
            std::mem::size_of::<u8>(),
        )?)
        .and_then(|total| {
            total.checked_add(
                runtime
                    .allocation_bytes(constraint.faces.capacity(), std::mem::size_of::<usize>())
                    .ok()?,
            )
        })
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let prior_capacity = constraints.capacity();
    let next_capacity = if constraints.len() == prior_capacity {
        next_vector_capacity(
            prior_capacity,
            constraints.len(),
            runtime.limits.max_constraints,
            runtime,
        )?
    } else {
        prior_capacity
    };
    let outer_bytes = runtime.allocation_bytes(
        next_capacity - prior_capacity,
        std::mem::size_of::<TupleConstraint>(),
    )?;
    let additional = outer_bytes
        .checked_add(nested_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let old_outer_bytes = if next_capacity > prior_capacity {
        runtime.allocation_bytes(prior_capacity, std::mem::size_of::<TupleConstraint>())?
    } else {
        0
    };
    let peak_additional = additional
        .checked_add(old_outer_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    ensure_constraint_scope_transient(runtime, storage_scope, peak_additional)?;
    add_constraint_problem_storage(runtime, storage_scope, additional)?;
    if next_capacity > prior_capacity {
        // A grow may allocate and move the new buffer before releasing the
        // old allocation. `peak_additional` admitted both buffers before the
        // retained accounting was committed.
        constraints
            .try_reserve_exact(next_capacity - constraints.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
    }
    constraints.push(constraint);
    Ok(())
}

struct RelationConstraintInput<'a> {
    kind: FacewiseConstraintKind,
    relations: &'a [(usize, usize)],
    faces: &'a [usize],
    supporting_cell: Option<OverlapCellKey>,
    variable_pairs: &'a [(usize, usize)],
}

fn relation_constraint<O, F>(
    input: RelationConstraintInput<'_>,
    accepts: F,
    runtime: &Runtime<'_, O>,
    storage_scope: ConstraintStorageScope,
) -> FacewiseResult<TupleConstraint>
where
    O: GlobalFlatFoldabilityObserver + ?Sized,
    F: Fn(&[bool]) -> bool,
{
    let RelationConstraintInput {
        kind,
        relations,
        faces,
        supporting_cell,
        variable_pairs,
    } = input;
    ensure_constraint_construction_headroom(runtime, storage_scope)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(relations.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for &(first, second) in relations {
        if first == second {
            return Err(FacewiseAbort::Execution(internal_error()));
        }
        let Ok(variable) = variable_pairs.binary_search(&ordered_pair(first, second)) else {
            return Err(certificate_failure());
        };
        variables.push(variable);
    }
    variables.sort_unstable();
    variables.dedup();
    if variables.len() > 6 {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let row_count = 1_u16
        .checked_shl(u32::try_from(variables.len()).map_err(|_| internal_abort())?)
        .ok_or_else(internal_abort)?;
    let mut allowed_rows = Vec::new();
    allowed_rows
        .try_reserve_exact(usize::from(row_count))
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for row in 0..row_count {
        let row = u8::try_from(row).map_err(|_| internal_abort())?;
        let mut relation_values = [false; 6];
        for (index, &(first, second)) in relations.iter().enumerate() {
            relation_values[index] =
                directed_face_above_from_row(first, second, row, &variables, variable_pairs)?;
        }
        if accepts(&relation_values[..relations.len()]) {
            allowed_rows.push(row);
        }
    }
    if allowed_rows.is_empty() {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    Ok(TupleConstraint {
        kind,
        variables,
        allowed_rows,
        faces: {
            let mut stored_faces = Vec::new();
            stored_faces.try_reserve_exact(faces.len()).map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
            stored_faces.extend_from_slice(faces);
            stored_faces
        },
        supporting_cell,
    })
}

fn directed_face_above_from_row(
    first: usize,
    second: usize,
    row: u8,
    variables: &[usize],
    variable_pairs: &[(usize, usize)],
) -> FacewiseResult<bool> {
    let pair = ordered_pair(first, second);
    let variable = variable_pairs
        .binary_search(&pair)
        .map_err(|_| certificate_failure())?;
    let position = variables
        .binary_search(&variable)
        .map_err(|_| certificate_failure())?;
    let canonical_second_above_first = row & (1_u8 << position) != 0;
    Ok(if first == pair.0 {
        !canonical_second_above_first
    } else {
        canonical_second_above_first
    })
}

fn supporting_cell<O: GlobalFlatFoldabilityObserver + ?Sized>(
    cells: &[OverlapCell],
    faces: &[usize],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Option<OverlapCellKey>> {
    let mut minimum = None;
    let mut control_poll = 0_usize;
    for cell in cells {
        runtime.poll_control(&mut control_poll)?;
        let mut supports_all = true;
        for face in faces {
            runtime.poll_control(&mut control_poll)?;
            if !cell.covering_faces.contains(face) {
                supports_all = false;
                break;
            }
        }
        if supports_all && minimum.is_none_or(|key: OverlapCellKey| cell.key.0 < key.0) {
            minimum = Some(cell.key);
        }
    }
    Ok(minimum)
}

fn all_face_pairs_overlap(faces: &[usize], variable_pairs: &[(usize, usize)]) -> bool {
    (0..faces.len()).all(|first| {
        ((first + 1)..faces.len()).all(|second| {
            variable_pairs
                .binary_search(&ordered_pair(faces[first], faces[second]))
                .is_ok()
        })
    })
}

fn compare_constraints(left: &TupleConstraint, right: &TupleConstraint) -> Ordering {
    constraint_kind_rank(left.kind)
        .cmp(&constraint_kind_rank(right.kind))
        .then_with(|| left.faces.cmp(&right.faces))
        .then_with(|| {
            left.supporting_cell
                .map(|cell| cell.0)
                .cmp(&right.supporting_cell.map(|cell| cell.0))
        })
        .then_with(|| left.variables.cmp(&right.variables))
        .then_with(|| left.allowed_rows.cmp(&right.allowed_rows))
}

const fn constraint_kind_rank(kind: FacewiseConstraintKind) -> u8 {
    match kind {
        FacewiseConstraintKind::Antisymmetry => 0,
        FacewiseConstraintKind::Transitivity => 1,
        FacewiseConstraintKind::TortillaTortilla => 2,
        FacewiseConstraintKind::TacoTortilla => 3,
        FacewiseConstraintKind::TacoTaco => 4,
        FacewiseConstraintKind::MountainValley => 5,
    }
}

fn segment_overlaps_face_interior<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &Point,
    second: &Point,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    if first == second {
        return Ok(false);
    }
    let mut lower = Rational::zero();
    let mut upper = Rational::from_integer(1.into());
    for edge_index in 0..polygon.len() {
        let edge_first = &polygon[edge_index];
        let edge_second = &polygon[(edge_index + 1) % polygon.len()];
        let first_side = cross(edge_first, edge_second, first, runtime)?;
        let second_side = cross(edge_first, edge_second, second, runtime)?;
        if first_side.is_zero() && second_side.is_zero() {
            return Ok(false);
        }
        if !first_side.is_positive() && !second_side.is_positive() {
            return Ok(false);
        }
        if first_side.is_negative() && !second_side.is_negative() {
            let crossing = div(
                &first_side,
                &sub(&first_side, &second_side, runtime)?,
                runtime,
            )?;
            if cmp(&crossing, &lower, runtime)? == Ordering::Greater {
                lower = crossing;
            }
        } else if !first_side.is_negative() && second_side.is_negative() {
            let crossing = div(
                &first_side,
                &sub(&first_side, &second_side, runtime)?,
                runtime,
            )?;
            if cmp(&crossing, &upper, runtime)? == Ordering::Less {
                upper = crossing;
            }
        }
        if cmp(&lower, &upper, runtime)? != Ordering::Less {
            return Ok(false);
        }
    }
    let clipped_first = interpolate(first, second, &lower, runtime)?;
    let clipped_second = interpolate(first, second, &upper, runtime)?;
    let representative = midpoint(&clipped_first, &clipped_second, runtime)?;
    point_in_convex_polygon(&representative, polygon, runtime)
}

fn segments_overlap_in_positive_length<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first_start: &Point,
    first_end: &Point,
    second_start: &Point,
    second_end: &Point,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    if first_start == first_end || second_start == second_end {
        return Ok(false);
    }
    if !cross(first_start, first_end, second_start, runtime)?.is_zero()
        || !cross(first_start, first_end, second_end, runtime)?.is_zero()
    {
        return Ok(false);
    }
    let (first_min, first_max, second_min, second_max) = if first_start.x != first_end.x {
        let (first_min, first_max) = ordered_rationals(&first_start.x, &first_end.x);
        let (second_min, second_max) = ordered_rationals(&second_start.x, &second_end.x);
        (first_min, first_max, second_min, second_max)
    } else {
        let (first_min, first_max) = ordered_rationals(&first_start.y, &first_end.y);
        let (second_min, second_max) = ordered_rationals(&second_start.y, &second_end.y);
        (first_min, first_max, second_min, second_max)
    };
    let lower = if cmp(first_min, second_min, runtime)? == Ordering::Greater {
        first_min
    } else {
        second_min
    };
    let upper = if cmp(first_max, second_max, runtime)? == Ordering::Less {
        first_max
    } else {
        second_max
    };
    Ok(cmp(lower, upper, runtime)? == Ordering::Less)
}

fn ordered_rationals<'a>(
    first: &'a Rational,
    second: &'a Rational,
) -> (&'a Rational, &'a Rational) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn constraint_contradiction(
    constraint: ConstraintView<'_>,
    embedding: &FlatEmbedding,
) -> FacewiseAbort {
    constraint_contradiction_parts(
        constraint.kind(),
        constraint.faces(),
        constraint.supporting_cell(),
        embedding,
    )
}

fn constraint_conflict_contradiction(
    conflict: ConstraintConflict,
    embedding: &FlatEmbedding,
) -> FacewiseAbort {
    constraint_contradiction_parts(
        conflict.kind,
        conflict.faces(),
        conflict.supporting_cell,
        embedding,
    )
}

fn constraint_contradiction_parts(
    kind: FacewiseConstraintKind,
    face_indices: &[usize],
    supporting_cell: Option<OverlapCellKey>,
    embedding: &FlatEmbedding,
) -> FacewiseAbort {
    let faces = face_indices
        .iter()
        .filter_map(|index| embedding.faces.get(*index))
        .map(|face| face.source.layer)
        .collect::<Vec<_>>();
    if faces.len() != face_indices.len() {
        return FacewiseAbort::Execution(internal_error());
    }
    FacewiseAbort::Impossible(
        GlobalFlatFoldabilityImpossibleReason::FacewiseConstraintContradiction {
            constraint_kind: kind,
            faces,
            supporting_cell,
        },
    )
}

fn certificate_failure() -> FacewiseAbort {
    FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
        reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
    })
}

fn verify_overlap_cell_interiors_are_disjoint<O: GlobalFlatFoldabilityObserver + ?Sized>(
    cells: &[OverlapCell],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    // Preserve control priority before reserving verifier-only acceleration
    // storage. Bounds retain only point indexes; the exact rationals remain
    // owned and accounted by the canonical overlap-cell boundaries.
    runtime.checkpoint(None)?;
    let verification_base = runtime.verification_storage_bytes();
    let bounds_bytes =
        runtime.allocation_bytes(cells.len(), std::mem::size_of::<ExactAxisAlignedBounds>())?;
    runtime.add_verification_storage(bounds_bytes)?;
    let verification = (|| {
        let mut bounds = Vec::new();
        bounds.try_reserve_exact(cells.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        let retained_bounds_bytes = runtime.allocation_bytes(
            bounds.capacity(),
            std::mem::size_of::<ExactAxisAlignedBounds>(),
        )?;
        runtime.restore_verification_storage(verification_base);
        runtime.add_verification_storage(retained_bounds_bytes)?;
        for cell in cells {
            runtime.checkpoint(None)?;
            bounds.push(exact_axis_aligned_bounds(&cell.boundary, runtime)?);
        }
        for first_cell in 0..cells.len() {
            runtime.checkpoint(None)?;
            for second_cell in (first_cell + 1)..cells.len() {
                if exact_axis_aligned_bounds_interiors_cannot_overlap(
                    &cells[first_cell].boundary,
                    bounds[first_cell],
                    &cells[second_cell].boundary,
                    bounds[second_cell],
                    runtime,
                )? {
                    continue;
                }
                if convex_polygon_interiors_overlap(
                    &cells[first_cell].boundary,
                    &cells[second_cell].boundary,
                    runtime,
                )? {
                    return Err(certificate_failure());
                }
            }
        }
        Ok(())
    })();
    runtime.restore_verification_storage(verification_base);
    verification
}

fn verify_facewise_certificate<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    pairs: &[OverlapPair],
    cells: &[OverlapCell],
    problem: &ConstraintProblem,
    assignment: &[bool],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    runtime.clear_verification_storage();
    verify_embedding_certificate(embedding, runtime)?;
    // Every verifier workspace value is local to
    // `verify_embedding_certificate` and has been dropped on return.
    runtime.clear_verification_storage();
    let verification_base = runtime.verification_storage_bytes();
    let regenerated = match build_constraint_problem(embedding, pairs, cells, runtime, false) {
        Ok(regenerated) => regenerated,
        Err(abort) => {
            runtime.restore_verification_storage(verification_base);
            return Err(abort);
        }
    };
    if regenerated != *problem || assignment.len() != problem.variables.len() {
        drop(regenerated);
        runtime.restore_verification_storage(verification_base);
        return Err(certificate_failure());
    }
    if problem
        .fixed_assignments
        .iter()
        .zip(assignment)
        .any(|(fixed, value)| fixed.is_some_and(|fixed| fixed != *value))
    {
        drop(regenerated);
        runtime.restore_verification_storage(verification_base);
        return Err(certificate_failure());
    }
    let verifier_memory_limit = runtime.remaining_storage_bytes()?;
    let retained_search_nodes = runtime.work.search_nodes;
    let verification = verify_complete_assignment_with_memory(
        assignment,
        &regenerated.constraints,
        verifier_memory_limit,
        |event, _| runtime.constraint_solver_control(event, retained_search_nodes),
    );
    match verification {
        CompleteAssignmentVerificationResult::Accepts => {}
        CompleteAssignmentVerificationResult::Rejects
        | CompleteAssignmentVerificationResult::InvalidConstraint => {
            drop(regenerated);
            runtime.restore_verification_storage(verification_base);
            return Err(certificate_failure());
        }
        CompleteAssignmentVerificationResult::DeadlineReached => {
            drop(regenerated);
            runtime.restore_verification_storage(verification_base);
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: runtime.phase,
                },
            ));
        }
        CompleteAssignmentVerificationResult::Cancelled => {
            drop(regenerated);
            runtime.restore_verification_storage(verification_base);
            return Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled,
            ));
        }
        CompleteAssignmentVerificationResult::WorkingMemoryLimit { observed } => {
            let used = runtime
                .exact_storage
                .total()
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            drop(regenerated);
            runtime.restore_verification_storage(verification_base);
            return Err(runtime.exact_storage_limit_failure(used.saturating_add(observed)));
        }
    }
    drop(regenerated);
    runtime.restore_verification_storage(verification_base);
    runtime.add_verification_storage(runtime.allocation_bytes(
        problem.variables.len(),
        std::mem::size_of::<((usize, usize), bool)>(),
    )?)?;
    let pair_values = PairValues::try_from_parallel(&problem.variables, assignment)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    if pair_values.len() != pairs.len() {
        return Err(certificate_failure());
    }
    verify_geometric_constraints_direct(embedding, cells, problem, &pair_values, runtime)?;
    let face_bounds = build_verifier_face_bounds(&embedding.faces, runtime)?;

    runtime.add_verification_storage(runtime.allocation_bytes(
        pair_values.len(),
        std::mem::size_of::<((usize, usize), Rational)>(),
    )?)?;
    let mut actual_pair_areas = Vec::new();
    actual_pair_areas
        .try_reserve_exact(pair_values.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for first in 0..embedding.faces.len() {
        runtime.checkpoint(None)?;
        for second in (first + 1)..embedding.faces.len() {
            if exact_axis_aligned_bounds_are_strictly_separated(
                &embedding.faces[first].polygon,
                face_bounds[first],
                &embedding.faces[second].polygon,
                face_bounds[second],
                runtime,
            )? {
                continue;
            }
            let intersection = convex_polygon_intersection(
                &embedding.faces[first].polygon,
                &embedding.faces[second].polygon,
                runtime,
            )?;
            if intersection.len() >= 3 {
                let area = signed_double_area(&intersection, runtime)?;
                if area.is_positive() {
                    let area_storage = exact::rational_storage_bytes(&area)?;
                    runtime.add_verification_storage(area_storage)?;
                    actual_pair_areas.push(((first, second), area));
                }
            }
        }
    }
    if pair_values
        .keys()
        .copied()
        .ne(actual_pair_areas.iter().map(|(pair, _)| *pair))
    {
        return Err(certificate_failure());
    }
    runtime.add_verification_storage(
        runtime.allocation_bytes(
            embedding
                .faces
                .len()
                .checked_mul(2)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            std::mem::size_of::<usize>(),
        )?,
    )?;
    let mut polygon_representatives = Vec::<usize>::new();
    polygon_representatives
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut polygon_classes = Vec::new();
    polygon_classes
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for (index, face) in embedding.faces.iter().enumerate() {
        let class = polygon_representatives
            .iter()
            .position(|representative| embedding.faces[*representative].polygon == face.polygon)
            .unwrap_or_else(|| {
                polygon_representatives.push(index);
                polygon_representatives.len() - 1
            });
        polygon_classes.push(class);
    }
    // No independent O(F^3) common-interior scan is needed here. The checks
    // below prove a stronger finite partition statement: every retained cell
    // is positive-area, strictly convex, and interior-disjoint; membership in
    // `covering_faces` is equivalent to exact polygon containment for every
    // coincident-polygon class; and the disjoint covering-cell areas sum to
    // each complete face area. Therefore the part of any positive-area
    // three-face common interior outside each face's verified cell union has
    // measure zero. A positive open common interior cannot be covered by the
    // union of those three measure-zero gaps, so one verified cell covers all
    // three faces. `order_cell_faces` then checks every pair in that cell's
    // total order, which independently excludes a directed three-cycle.
    let cell_key_entry_bytes = std::mem::size_of::<[u8; 32]>()
        .checked_add(3 * std::mem::size_of::<usize>())
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime
        .add_verification_storage(runtime.allocation_bytes(cells.len(), cell_key_entry_bytes)?)?;
    let mut cell_keys = HashSet::new();
    cell_keys
        .try_reserve(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(cells.len(), std::mem::size_of::<Rational>())?,
    )?;
    let mut verified_cell_areas = Vec::new();
    verified_cell_areas
        .try_reserve_exact(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in cells {
        runtime.checkpoint(None)?;
        let cell_scope_base = runtime.verification_storage_bytes();
        let covering_set_bytes = runtime.allocation_bytes(
            cell.covering_faces.len(),
            std::mem::size_of::<usize>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        let boundary_set_structure = runtime.allocation_bytes(
            cell.boundary.len(),
            std::mem::size_of::<Point>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        let boundary_set_exact = exact_storage_bytes_points(&cell.boundary)?;
        let ordered_face_bytes =
            runtime.allocation_bytes(cell.covering_faces.len(), std::mem::size_of::<usize>())?;
        let cell_temporary_bytes = covering_set_bytes
            .checked_add(boundary_set_structure)
            .and_then(|total| total.checked_add(boundary_set_exact))
            .and_then(|total| total.checked_add(ordered_face_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        runtime.add_verification_storage(cell_temporary_bytes)?;
        let covering_faces_are_unique = cell
            .covering_faces
            .windows(2)
            .all(|faces| faces[0] < faces[1]);
        let mut boundary_points_are_unique = true;
        let mut uniqueness_poll = 0_usize;
        for first in 0..cell.boundary.len() {
            for second in (first + 1)..cell.boundary.len() {
                runtime.poll_control(&mut uniqueness_poll)?;
                if cell.boundary[first] == cell.boundary[second] {
                    boundary_points_are_unique = false;
                    break;
                }
            }
            if !boundary_points_are_unique {
                break;
            }
        }
        let mut strictly_convex = cell.boundary.len() >= 3;
        for index in 0..cell.boundary.len() {
            strictly_convex &= cross(
                &cell.boundary[index],
                &cell.boundary[(index + 1) % cell.boundary.len()],
                &cell.boundary[(index + 2) % cell.boundary.len()],
                runtime,
            )?
            .is_positive();
        }
        let cell_area = signed_double_area(&cell.boundary, runtime)?;
        if cell.boundary.len() < 3
            || !cell_area.is_positive()
            || overlap_cell_key(
                &cell.boundary,
                &cell.covering_faces,
                &embedding.faces,
                runtime,
            )? != cell.key
            || !cell_keys.insert(cell.key.0)
            || !boundary_points_are_unique
            || !strictly_convex
            || !covering_faces_are_unique
        {
            return Err(certificate_failure());
        }
        let cell_bounds = exact_axis_aligned_bounds(&cell.boundary, runtime)?;
        for (class_index, &representative) in polygon_representatives.iter().enumerate() {
            let face_covers_cell = consistent_polygon_class_coverage(
                &cell.covering_faces,
                &polygon_classes,
                class_index,
                representative,
                runtime,
            )?;
            let face = &embedding.faces[representative];
            if exact_axis_aligned_bounds_interiors_cannot_overlap(
                &cell.boundary,
                cell_bounds,
                &face.polygon,
                face_bounds[representative],
                runtime,
            )? {
                if face_covers_cell {
                    return Err(certificate_failure());
                }
                continue;
            }
            if face_covers_cell {
                if !convex_polygon_contains_polygon(&face.polygon, &cell.boundary, runtime)? {
                    return Err(certificate_failure());
                }
            } else if convex_polygon_interiors_overlap(&cell.boundary, &face.polygon, runtime)? {
                return Err(certificate_failure());
            }
        }
        let ordered_faces = order_cell_faces(&cell.covering_faces, &pair_values, runtime)?;
        if ordered_faces.len() != cell.covering_faces.len() {
            return Err(certificate_failure());
        }
        let mut order_verification_poll = 0_usize;
        for lower_index in 0..ordered_faces.len() {
            for upper_index in (lower_index + 1)..ordered_faces.len() {
                runtime.poll_control(&mut order_verification_poll)?;
                if !face_is_below(
                    ordered_faces[lower_index],
                    ordered_faces[upper_index],
                    &pair_values,
                )? {
                    return Err(certificate_failure());
                }
            }
        }
        let retained_cell_area_bytes = exact::rational_storage_bytes(&cell_area)?;
        drop(ordered_faces);
        runtime.restore_verification_storage(cell_scope_base);
        runtime.add_verification_storage(retained_cell_area_bytes)?;
        verified_cell_areas.push(cell_area);
    }
    verify_overlap_cell_interiors_are_disjoint(cells, runtime)?;
    for ((first, second), expected_area) in &actual_pair_areas {
        runtime.checkpoint(None)?;
        let mut covered_area = Rational::zero();
        for (cell, area) in cells.iter().zip(&verified_cell_areas) {
            if cell.covering_faces.contains(first) && cell.covering_faces.contains(second) {
                covered_area = add(&covered_area, area, runtime)?;
                runtime.ensure_transient_exact_storage(exact::rational_storage_bytes(
                    &covered_area,
                )?)?;
            }
        }
        if &covered_area != expected_area {
            return Err(certificate_failure());
        }
    }
    for (face_index, face) in embedding.faces.iter().enumerate() {
        runtime.checkpoint(None)?;
        let expected_area = signed_double_area(&face.polygon, runtime)?;
        let mut covered_area = Rational::zero();
        for (cell, area) in cells.iter().zip(&verified_cell_areas) {
            if cell.covering_faces.contains(&face_index) {
                covered_area = add(&covered_area, area, runtime)?;
                runtime.ensure_transient_exact_storage(exact::rational_storage_bytes(
                    &covered_area,
                )?)?;
            }
        }
        if covered_area != expected_area {
            return Err(certificate_failure());
        }
    }
    for &((first, second), _) in &actual_pair_areas {
        if supporting_cell(cells, &[first, second], runtime)?.is_none() {
            return Err(certificate_failure());
        }
    }
    drop(verified_cell_areas);
    drop(cell_keys);
    drop(actual_pair_areas);
    drop(pair_values);
    drop(face_bounds);
    runtime.clear_verification_storage();
    runtime.checkpoint(None)?;
    Ok(())
}

fn verify_embedding_certificate<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    if embedding.faces.is_empty()
        || embedding.reference_face != 0
        || embedding.faces[0].transform != Transform::identity()
        || !embedding.faces[0].front_up
        || embedding.material_internal_edge_count != embedding.hinges.len()
    {
        return Err(certificate_failure());
    }
    let face_set_entry_bytes = std::mem::size_of::<ori_domain::FaceId>()
        .checked_add(std::mem::size_of::<ori_topology::FaceKey>())
        .and_then(|total| total.checked_add(4 * std::mem::size_of::<usize>()))
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(embedding.faces.len(), face_set_entry_bytes)?,
    )?;
    let mut face_ids = HashSet::new();
    face_ids
        .try_reserve(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut face_keys = HashSet::new();
    face_keys
        .try_reserve(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for (face_index, face) in embedding.faces.iter().enumerate() {
        runtime.checkpoint(None)?;
        if !face_ids.insert(face.source.layer.face_id)
            || !face_keys.insert(face.source.layer.face_key)
            || (face_index > 0
                && embedding.faces[face_index - 1].source.layer.face_key
                    >= face.source.layer.face_key)
        {
            return Err(certificate_failure());
        }
        let determinant = sub(
            &mul(&face.transform.m00, &face.transform.m11, runtime)?,
            &mul(&face.transform.m01, &face.transform.m10, runtime)?,
            runtime,
        )?;
        if determinant.is_zero() || determinant.is_positive() != face.front_up {
            return Err(certificate_failure());
        }
        let recomputed_structure_bytes = runtime.allocation_bytes(
            face.source.source_polygon.len(),
            std::mem::size_of::<Point>(),
        )?;
        runtime.ensure_transient_exact_storage(recomputed_structure_bytes)?;
        let mut recomputed_polygon = Vec::new();
        recomputed_polygon
            .try_reserve_exact(face.source.source_polygon.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        let mut recomputed_exact_bytes = 0_usize;
        let mut recompute_poll = 0_usize;
        for source_point in &face.source.source_polygon {
            runtime.poll_control(&mut recompute_poll)?;
            let recomputed_point = apply(&face.transform, source_point, runtime)?;
            recomputed_exact_bytes = recomputed_exact_bytes
                .checked_add(exact_storage_bytes_point(&recomputed_point)?)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            runtime.ensure_transient_exact_storage(
                recomputed_structure_bytes
                    .checked_add(recomputed_exact_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            recomputed_polygon.push(recomputed_point);
        }
        let area = signed_double_area(&recomputed_polygon, runtime)?;
        runtime.ensure_transient_exact_storage(
            recomputed_structure_bytes
                .checked_add(recomputed_exact_bytes)
                .and_then(|total| total.checked_add(exact::rational_storage_bytes(&area).ok()?))
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        if area.is_negative() {
            recomputed_polygon.reverse();
        }
        if area.is_zero() || recomputed_polygon != face.polygon {
            return Err(certificate_failure());
        }
    }
    let hinge_entry_bytes = std::mem::size_of::<EdgeId>()
        .checked_add(2 * std::mem::size_of::<usize>())
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let connectivity_bytes = runtime
        .allocation_bytes(embedding.hinges.len(), hinge_entry_bytes)?
        .checked_add(runtime.allocation_bytes(embedding.faces.len(), std::mem::size_of::<usize>())?)
        .and_then(|total| {
            total.checked_add(
                runtime
                    .allocation_bytes(embedding.faces.len(), std::mem::size_of::<u8>())
                    .ok()?,
            )
        })
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.add_verification_storage(connectivity_bytes)?;
    let mut hinge_edges = HashSet::new();
    hinge_edges
        .try_reserve(embedding.hinges.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut parents = Vec::new();
    parents
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    parents.extend(0..embedding.faces.len());
    let mut ranks = Vec::new();
    ranks
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    ranks.resize(embedding.faces.len(), 0_u8);
    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        if hinge.first_face >= embedding.faces.len()
            || hinge.second_face >= embedding.faces.len()
            || hinge.first_face == hinge.second_face
            || hinge.first_point == hinge.second_point
            || !hinge_edges.insert(hinge.edge)
            || !polygon_has_edge(
                &embedding.faces[hinge.first_face].polygon,
                &hinge.first_point,
                &hinge.second_point,
            )
            || !polygon_has_edge(
                &embedding.faces[hinge.second_face].polygon,
                &hinge.first_point,
                &hinge.second_point,
            )
        {
            return Err(certificate_failure());
        }
        let reflection = reflection_across(&hinge.first_point, &hinge.second_point, runtime)?;
        let expected_second = compose(
            &reflection,
            &embedding.faces[hinge.first_face].transform,
            runtime,
        )?;
        if expected_second != embedding.faces[hinge.second_face].transform {
            return Err(certificate_failure());
        }
        union_face_components(
            &mut parents,
            &mut ranks,
            hinge.first_face,
            hinge.second_face,
        );
    }
    let reference_root = find_face_component_root(&mut parents, embedding.reference_face);
    let mut connectivity_poll = 0_usize;
    for face in 0..embedding.faces.len() {
        runtime.poll_control(&mut connectivity_poll)?;
        if find_face_component_root(&mut parents, face) != reference_root {
            return Err(certificate_failure());
        }
    }
    Ok(())
}

fn find_face_component_root(parents: &mut [usize], face: usize) -> usize {
    let mut root = face;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = face;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_face_components(parents: &mut [usize], ranks: &mut [u8], first: usize, second: usize) {
    let first_root = find_face_component_root(parents, first);
    let second_root = find_face_component_root(parents, second);
    if first_root == second_root {
        return;
    }
    match ranks[first_root].cmp(&ranks[second_root]) {
        Ordering::Less => parents[first_root] = second_root,
        Ordering::Greater => parents[second_root] = first_root,
        Ordering::Equal => {
            parents[second_root] = first_root;
            ranks[first_root] = ranks[first_root].saturating_add(1);
        }
    }
}

fn polygon_has_edge(polygon: &[Point], first: &Point, second: &Point) -> bool {
    (0..polygon.len()).any(|index| {
        let current = &polygon[index];
        let next = &polygon[(index + 1) % polygon.len()];
        (current == first && next == second) || (current == second && next == first)
    })
}

fn verify_geometric_constraints_direct<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    cells: &[OverlapCell],
    problem: &ConstraintProblem,
    pair_values: &PairValues,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    if embedding.material_internal_edge_count != embedding.hinges.len() {
        return Err(certificate_failure());
    }
    let mut expected_constraint_count = pair_values.len();
    runtime.add_verification_storage(
        runtime.allocation_bytes(
            embedding
                .faces
                .len()
                .checked_mul(2)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            std::mem::size_of::<usize>(),
        )?,
    )?;
    let mut polygon_representatives = Vec::<usize>::new();
    polygon_representatives
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut polygon_classes = Vec::new();
    polygon_classes
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for (index, face) in embedding.faces.iter().enumerate() {
        let class = polygon_representatives
            .iter()
            .position(|representative| embedding.faces[*representative].polygon == face.polygon)
            .unwrap_or_else(|| {
                polygon_representatives.push(index);
                polygon_representatives.len() - 1
            });
        polygon_classes.push(class);
    }
    let same_segment = |first: &FoldedHinge, second: &FoldedHinge| {
        (first.first_point == second.first_point && first.second_point == second.second_point)
            || (first.first_point == second.second_point
                && first.second_point == second.first_point)
    };
    runtime.add_verification_storage(
        runtime.allocation_bytes(
            embedding
                .hinges
                .len()
                .checked_mul(2)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            std::mem::size_of::<usize>(),
        )?,
    )?;
    let mut segment_representatives = Vec::<usize>::new();
    segment_representatives
        .try_reserve_exact(embedding.hinges.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut segment_classes = Vec::new();
    segment_classes
        .try_reserve_exact(embedding.hinges.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for (index, hinge) in embedding.hinges.iter().enumerate() {
        let class = segment_representatives
            .iter()
            .position(|representative| same_segment(&embedding.hinges[*representative], hinge))
            .unwrap_or_else(|| {
                segment_representatives.push(index);
                segment_representatives.len() - 1
            });
        segment_classes.push(class);
    }
    let mut segment_face_overlap = HashMap::<(usize, usize), bool>::new();
    let mut segment_pair_overlap = HashMap::<(usize, usize), bool>::new();

    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        expected_constraint_count = checked_certificate_count(expected_constraint_count, 1)?;
        let expected_canonical = mountain_valley_canonical_value(
            hinge.assignment,
            embedding.faces[hinge.first_face].front_up,
            hinge.first_face,
            hinge.second_face,
        );
        if pair_values
            .get(&ordered_pair(hinge.first_face, hinge.second_face))
            .copied()
            != Some(expected_canonical)
        {
            return Err(certificate_failure());
        }
    }

    // A tournament is transitive exactly when its outdegrees are the distinct
    // values `0..ply`.  Checking that score sequence is O(ply^2), whereas
    // enumerating every forbidden directed triangle is O(ply^3).  Keep the
    // logical certificate count unchanged: the compact family still denotes
    // every C(ply, 3) constraint.
    let mut maximum_cell_ply = 0_usize;
    let mut maximum_ply_poll = 0_usize;
    for cell in cells {
        runtime.poll_control(&mut maximum_ply_poll)?;
        maximum_cell_ply = maximum_cell_ply.max(cell.covering_faces.len());
    }
    runtime.ensure_transient_exact_storage(
        runtime.allocation_bytes(maximum_cell_ply, std::mem::size_of::<usize>())?,
    )?;
    let mut tournament_outdegrees = Vec::new();
    tournament_outdegrees
        .try_reserve_exact(maximum_cell_ply)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut transitivity_poll = 0_usize;
    for cell in cells {
        runtime.checkpoint(None)?;
        let ply = cell.covering_faces.len();
        expected_constraint_count = checked_certificate_count(
            expected_constraint_count,
            choose_three(ply).ok_or_else(certificate_failure)?,
        )?;
        if ply < 3 {
            continue;
        }
        tournament_outdegrees.clear();
        tournament_outdegrees.resize(ply, 0_usize);
        for first_index in 0..ply {
            for second_index in (first_index + 1)..ply {
                runtime.poll_control(&mut transitivity_poll)?;
                let first = cell.covering_faces[first_index];
                let second = cell.covering_faces[second_index];
                let winner = if face_above(first, second, pair_values)? {
                    first_index
                } else {
                    second_index
                };
                tournament_outdegrees[winner] = tournament_outdegrees[winner]
                    .checked_add(1)
                    .ok_or_else(certificate_failure)?;
            }
        }
        if !transitive_tournament_degree_sequence(&mut tournament_outdegrees) {
            return Err(certificate_failure());
        }
    }
    drop(tournament_outdegrees);

    for (hinge_index, hinge) in embedding.hinges.iter().enumerate() {
        runtime.checkpoint(None)?;
        for (face, &polygon_class) in polygon_classes.iter().enumerate() {
            if face == hinge.first_face || face == hinge.second_face {
                continue;
            }
            let predicate_key = (segment_classes[hinge_index], polygon_class);
            let overlaps = if let Some(value) = segment_face_overlap.get(&predicate_key) {
                *value
            } else {
                let value = segment_overlaps_face_interior(
                    &hinge.first_point,
                    &hinge.second_point,
                    &embedding.faces[face].polygon,
                    runtime,
                )?;
                runtime.add_verification_storage(
                    runtime
                        .allocation_bytes(1, 3 * std::mem::size_of::<((usize, usize), bool)>())?,
                )?;
                segment_face_overlap.try_reserve(1).map_err(|_| {
                    runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
                })?;
                segment_face_overlap.insert(predicate_key, value);
                value
            };
            if !overlaps {
                continue;
            }
            let mut evidence_faces = vec![hinge.first_face, hinge.second_face, face];
            evidence_faces.sort_unstable();
            if supporting_cell(cells, &evidence_faces, runtime)?.is_none()
                || face_above(hinge.first_face, face, pair_values)?
                    != face_above(hinge.second_face, face, pair_values)?
            {
                return Err(certificate_failure());
            }
            expected_constraint_count = checked_certificate_count(expected_constraint_count, 1)?;
        }
    }

    for first_hinge_index in 0..embedding.hinges.len() {
        runtime.checkpoint(None)?;
        for second_hinge_index in (first_hinge_index + 1)..embedding.hinges.len() {
            let first_hinge = &embedding.hinges[first_hinge_index];
            let second_hinge = &embedding.hinges[second_hinge_index];
            let first_class = segment_classes[first_hinge_index];
            let second_class = segment_classes[second_hinge_index];
            let predicate_key = if first_class <= second_class {
                (first_class, second_class)
            } else {
                (second_class, first_class)
            };
            let overlaps = if let Some(value) = segment_pair_overlap.get(&predicate_key) {
                *value
            } else {
                let value = segments_overlap_in_positive_length(
                    &first_hinge.first_point,
                    &first_hinge.second_point,
                    &second_hinge.first_point,
                    &second_hinge.second_point,
                    runtime,
                )?;
                runtime.add_verification_storage(
                    runtime
                        .allocation_bytes(1, 3 * std::mem::size_of::<((usize, usize), bool)>())?,
                )?;
                segment_pair_overlap.try_reserve(1).map_err(|_| {
                    runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
                })?;
                segment_pair_overlap.insert(predicate_key, value);
                value
            };
            if !overlaps {
                continue;
            }
            let mut evidence_faces = vec![
                first_hinge.first_face,
                first_hinge.second_face,
                second_hinge.first_face,
                second_hinge.second_face,
            ];
            evidence_faces.sort_unstable();
            evidence_faces.dedup();
            if evidence_faces.len() != 4 {
                return Err(certificate_failure());
            }
            if !all_face_pairs_assigned(&evidence_faces, pair_values) {
                continue;
            }
            if supporting_cell(cells, &evidence_faces, runtime)?.is_none() {
                return Err(certificate_failure());
            }
            let relations = [
                face_above(first_hinge.first_face, first_hinge.second_face, pair_values)?,
                face_above(
                    second_hinge.first_face,
                    second_hinge.second_face,
                    pair_values,
                )?,
                face_above(
                    second_hinge.first_face,
                    first_hinge.second_face,
                    pair_values,
                )?,
                face_above(
                    first_hinge.first_face,
                    second_hinge.second_face,
                    pair_values,
                )?,
                face_above(first_hinge.first_face, second_hinge.first_face, pair_values)?,
                face_above(
                    first_hinge.second_face,
                    second_hinge.second_face,
                    pair_values,
                )?,
            ];
            if !taco_taco_source_tuple_accepts(&relations) {
                return Err(certificate_failure());
            }
            expected_constraint_count = checked_certificate_count(expected_constraint_count, 1)?;
        }
    }

    if expected_constraint_count != problem.constraints.len() {
        return Err(certificate_failure());
    }
    Ok(())
}

fn checked_certificate_count(current: usize, additional: usize) -> FacewiseResult<usize> {
    current
        .checked_add(additional)
        .ok_or_else(certificate_failure)
}

fn transitive_tournament_degree_sequence(outdegrees: &mut [usize]) -> bool {
    let vertex_count = outdegrees.len();
    if vertex_count == 0 {
        return true;
    }
    if outdegrees.iter().any(|degree| *degree >= vertex_count) {
        return false;
    }
    // Mark an observed degree in place by adding `vertex_count`.  Reading a
    // later entry modulo `vertex_count` preserves its original degree even if
    // that slot was used as an earlier marker, so no second O(ply) buffer is
    // required.
    for index in 0..vertex_count {
        let degree = outdegrees[index] % vertex_count;
        if outdegrees[degree] >= vertex_count {
            return false;
        }
        let Some(marked) = outdegrees[degree].checked_add(vertex_count) else {
            return false;
        };
        outdegrees[degree] = marked;
    }
    true
}

fn all_face_pairs_assigned(faces: &[usize], pair_values: &PairValues) -> bool {
    (0..faces.len()).all(|first| {
        ((first + 1)..faces.len())
            .all(|second| pair_values.contains_key(&ordered_pair(faces[first], faces[second])))
    })
}

fn face_above(first: usize, second: usize, pair_values: &PairValues) -> FacewiseResult<bool> {
    let pair = ordered_pair(first, second);
    let canonical_second_above_first = pair_values
        .get(&pair)
        .copied()
        .ok_or_else(certificate_failure)?;
    Ok(if first == pair.0 {
        !canonical_second_above_first
    } else {
        canonical_second_above_first
    })
}

struct CertificateByteCounter<'runtime, 'observer, O: GlobalFlatFoldabilityObserver + ?Sized> {
    runtime: &'runtime mut Runtime<'observer, O>,
    limit: usize,
    observed: usize,
    bytes_since_poll: usize,
    exceeded: bool,
    abort: Option<FacewiseAbort>,
}

impl<O: GlobalFlatFoldabilityObserver + ?Sized> Write for CertificateByteCounter<'_, '_, O> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(observed) = self.observed.checked_add(bytes.len()) else {
            self.observed = usize::MAX;
            self.exceeded = true;
            return Err(io::Error::other("certificate byte count overflowed"));
        };
        self.observed = observed;
        if self.observed > self.limit {
            self.exceeded = true;
            return Err(io::Error::other("certificate byte limit reached"));
        }
        self.bytes_since_poll = self.bytes_since_poll.saturating_add(bytes.len());
        if self.bytes_since_poll >= SERIALIZATION_POLL_BYTES {
            self.bytes_since_poll = 0;
            if let Err(abort) = self.runtime.checkpoint(None) {
                self.abort = Some(abort);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "certificate serialization interrupted",
                ));
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialized_certificate_size<O: GlobalFlatFoldabilityObserver + ?Sized>(
    layer_order: &LayerOrderSnapshot,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<usize> {
    runtime.checkpoint(None)?;
    let limit = runtime.limits.max_certificate_bytes;
    let serialization_result;
    let observed;
    let exceeded;
    let abort;
    {
        let mut counter = CertificateByteCounter {
            runtime,
            limit,
            observed: 0,
            bytes_since_poll: 0,
            exceeded: false,
            abort: None,
        };
        serialization_result = serde_json::to_writer(&mut counter, layer_order);
        observed = counter.observed;
        exceeded = counter.exceeded;
        abort = counter.abort.take();
    }
    if let Some(abort) = abort {
        return Err(abort);
    }
    if serialization_result.is_err() {
        if exceeded {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                },
            ));
        }
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    runtime.checkpoint(None)?;
    Ok(observed)
}

fn finalize_certificate_size<O: GlobalFlatFoldabilityObserver + ?Sized>(
    layer_order: &mut LayerOrderSnapshot,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let mut prior = 0_usize;
    for _ in 0..4 {
        let Some(summary) = layer_order.proof_summary.as_mut() else {
            return Err(FacewiseAbort::Execution(internal_error()));
        };
        summary.certificate_bytes = prior;
        let observed = serialized_certificate_size(layer_order, runtime)?;
        if observed == prior {
            runtime.set_certificate_bytes(observed)?;
            runtime.checkpoint(None)?;
            return Ok(());
        }
        prior = observed;
    }
    Err(FacewiseAbort::Execution(internal_error()))
}

const fn ordered_pair(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn order_cell_faces<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[usize],
    pair_values: &PairValues,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<usize>> {
    let ordered_bytes = runtime.allocation_bytes(faces.len(), std::mem::size_of::<usize>())?;
    runtime.ensure_transient_exact_storage(ordered_bytes)?;
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    ordered.extend_from_slice(faces);
    let mut control_poll = 0_usize;
    for index in 1..ordered.len() {
        let mut cursor = index;
        while cursor > 0 {
            runtime.poll_control(&mut control_poll)?;
            if !face_is_below(ordered[cursor], ordered[cursor - 1], pair_values)? {
                break;
            }
            ordered.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
    runtime.checkpoint(None)?;
    Ok(ordered)
}

fn face_is_below(first: usize, second: usize, pair_values: &PairValues) -> FacewiseResult<bool> {
    let key = ordered_pair(first, second);
    let Some(second_canonical_above_first) = pair_values.get(&key).copied() else {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
            },
        ));
    };
    Ok(if first == key.0 {
        second_canonical_above_first
    } else {
        !second_canonical_above_first
    })
}

fn canonical_global_linear_extension<O: GlobalFlatFoldabilityObserver + ?Sized>(
    face_count: usize,
    pair_values: &PairValues,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Option<Vec<usize>>> {
    runtime.checkpoint(None)?;
    let face_header_bytes = runtime.allocation_bytes(
        face_count,
        std::mem::size_of::<Vec<usize>>()
            .checked_add(3 * std::mem::size_of::<usize>())
            .and_then(|total| total.checked_add(std::mem::size_of::<u8>()))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    let edge_bytes = runtime.allocation_bytes(pair_values.len(), std::mem::size_of::<usize>())?;
    let working_bytes = face_header_bytes
        .checked_add(edge_bytes)
        .and_then(|total| {
            total.checked_add(face_count.saturating_mul(2 * std::mem::size_of::<usize>()))
        })
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.ensure_transient_exact_storage(working_bytes)?;
    let mut outdegrees = Vec::new();
    outdegrees
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    outdegrees.resize(face_count, 0_usize);
    for &((first, second), second_above_first) in pair_values.iter() {
        let lower = if second_above_first { first } else { second };
        outdegrees[lower] = outdegrees[lower]
            .checked_add(1)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    }
    let mut outgoing = Vec::new();
    outgoing
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for degree in outdegrees {
        let mut neighbors = Vec::new();
        neighbors.try_reserve_exact(degree).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        outgoing.push(neighbors);
    }
    let mut indegree = Vec::new();
    indegree
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    indegree.resize(face_count, 0_usize);
    let mut control_poll = 0_usize;
    for &((first, second), second_above_first) in pair_values.iter() {
        runtime.poll_control(&mut control_poll)?;
        let (lower, upper) = if second_above_first {
            (first, second)
        } else {
            (second, first)
        };
        outgoing[lower].push(upper);
        indegree[upper] += 1;
    }
    for neighbors in &mut outgoing {
        neighbors.sort_unstable();
    }
    let mut ready = Vec::new();
    ready
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    ready.extend(indegree.iter().map(|degree| *degree == 0));
    let mut result = Vec::new();
    result
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    while let Some(current) = (0..face_count).find(|index| ready[*index]) {
        runtime.poll_control(&mut control_poll)?;
        ready[current] = false;
        result.push(current);
        for upper in outgoing[current].iter().copied() {
            runtime.poll_control(&mut control_poll)?;
            indegree[upper] -= 1;
            if indegree[upper] == 0 {
                ready[upper] = true;
            }
        }
    }
    if result.len() != face_count {
        // Facewise orders are location-dependent. A cycle assembled from
        // disjoint overlap cells is not a physical contradiction.
        runtime.checkpoint(None)?;
        return Ok(None);
    }
    runtime.checkpoint(None)?;
    Ok(Some(result))
}

#[cfg(test)]
#[path = "facewise/tests.rs"]
mod tests;
