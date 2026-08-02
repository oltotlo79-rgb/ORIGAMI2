use super::*;

pub const GLOBAL_FLAT_LAYER_ORDER_PAIR_REGISTRY_DOMAIN_V2: &[u8] =
    b"origami2/general-n-layer-pair-registry/v1";

pub const DEFAULT_MAX_COMPACT_PAIR_ASSIGNMENT_BYTES_V2: usize = 64 * 1024 * 1024;
pub const DEFAULT_MAX_COMPACT_LAYER_ORDER_RETAINED_BYTES_V2: usize = 128 * 1024 * 1024;
pub const DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2: usize = 256 * 1024 * 1024;

/// Untrusted compact direction assignment bound to one canonical live pair
/// registry. Bit `i` is little-endian within its byte. A set bit means that
/// the canonical first face is lower than the canonical second face.
#[derive(Debug, Clone, Copy)]
pub struct GlobalFlatLayerOrderCompactPairAssignmentInputV2<'a> {
    pub source: GlobalFlatFoldabilityInput<'a>,
    pub variable_count: usize,
    pub variable_registry_sha256: [u8; 32],
    pub direction_bits_le: &'a [u8],
}

/// Explicit finite bounds for no-search reconstruction from a compact pair
/// assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
    pub analysis: GlobalFlatFoldabilityLimits,
    pub max_compact_assignment_bytes: usize,
    pub max_layer_order_retained_bytes: usize,
    pub max_peak_bytes: usize,
}

impl Default for GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
    fn default() -> Self {
        Self {
            analysis: GlobalFlatFoldabilityLimits::default(),
            max_compact_assignment_bytes: DEFAULT_MAX_COMPACT_PAIR_ASSIGNMENT_BYTES_V2,
            max_layer_order_retained_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_RETAINED_BYTES_V2,
            max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalFlatLayerOrderCompactPairAssignmentMalformedV2 {
    ByteLength { expected: usize, actual: usize },
    NonZeroTailBits,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GlobalFlatLayerOrderCompactPairAssignmentErrorV2 {
    #[error("compact pair-assignment limits must all be finite")]
    InvalidLimits,
    #[error("the compact pair assignment is malformed: {0:?}")]
    Malformed(GlobalFlatLayerOrderCompactPairAssignmentMalformedV2),
    #[error("the declared canonical pair registry does not match the live source")]
    RegistryMismatch,
    #[error("the complete compact assignment violates the live flat-foldability constraints")]
    AssignmentRejected,
    #[error("the live source is inconclusive under the supplied limits")]
    Inconclusive {
        reason: GlobalFlatFoldabilityUnknownReason,
    },
    #[error("the live source violates a necessary flat-foldability condition")]
    LiveSourceImpossible,
    #[error("compact layer-order reconstruction could not complete: {0}")]
    Execution(#[from] GlobalFlatFoldabilityExecutionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalFlatLayerOrderCompactPairAssignmentResourcesV2 {
    pub compact_assignment_bytes: usize,
    pub borrowed_live_bytes: usize,
    pub layer_order_retained_bytes: usize,
    pub observed_validation_peak_bytes: usize,
    pub observed_facewise_peak_bytes: usize,
    pub observed_peak_bytes: usize,
}

/// Opaque, owned authority issued only after rebuilding and verifying the live
/// geometry, canonical pair registry, constraints, and complete layer-order
/// snapshot without invoking the completion search.
///
/// ```compile_fail
/// use ori_foldability::GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2>();
/// ```
///
/// ```compile_fail
/// use ori_foldability::GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2>();
/// ```
#[derive(Debug)]
pub struct GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2 {
    layer_order: LayerOrderSnapshot,
    provenance: GlobalFlatFoldabilityProvenance,
    work_counts: GlobalFlatFoldabilityWorkCounts,
    variable_count: usize,
    variable_registry_sha256: [u8; 32],
    limits: GlobalFlatLayerOrderCompactPairAssignmentLimitsV2,
    resources: GlobalFlatLayerOrderCompactPairAssignmentResourcesV2,
    _authority_seal: GlobalFlatLayerOrderCompactPairAssignmentSealV2,
}

#[derive(Debug)]
struct GlobalFlatLayerOrderCompactPairAssignmentSealV2;

impl GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2 {
    #[must_use]
    pub const fn layer_order_snapshot_v2(&self) -> &LayerOrderSnapshot {
        &self.layer_order
    }

    #[must_use]
    pub const fn provenance_v2(&self) -> GlobalFlatFoldabilityProvenance {
        self.provenance
    }

    #[must_use]
    pub const fn work_counts_v2(&self) -> GlobalFlatFoldabilityWorkCounts {
        self.work_counts
    }

    #[must_use]
    pub const fn variable_count_v2(&self) -> usize {
        self.variable_count
    }

    #[must_use]
    pub const fn variable_registry_sha256_v2(&self) -> [u8; 32] {
        self.variable_registry_sha256
    }

    #[must_use]
    pub const fn exact_limits_v2(&self) -> GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        self.limits
    }

    #[must_use]
    pub const fn resources_v2(&self) -> GlobalFlatLayerOrderCompactPairAssignmentResourcesV2 {
        self.resources
    }

    /// Rebuilds the current live geometry and revalidates this owned snapshot
    /// through the same no-search boundary used for an untrusted public
    /// certificate. Provenance equality alone is deliberately insufficient.
    pub fn revalidate_live_source_v2(
        &self,
        input: GlobalFlatFoldabilityInput<'_>,
        limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    ) -> Result<GlobalFlatLayerOrderSourceAuthorityV2<'_>, GlobalFlatLayerOrderRevalidationErrorV2>
    {
        revalidate_global_flat_layer_order_source_v2(input, &self.layer_order, limits)
    }
}

pub fn issue_global_flat_layer_order_from_compact_pair_assignment_v2(
    input: GlobalFlatLayerOrderCompactPairAssignmentInputV2<'_>,
    limits: GlobalFlatLayerOrderCompactPairAssignmentLimitsV2,
) -> Result<
    GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    GlobalFlatLayerOrderCompactPairAssignmentErrorV2,
> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    issue_global_flat_layer_order_from_compact_pair_assignment_with_observer_v2(
        input,
        limits,
        &mut observer,
    )
}

pub fn issue_global_flat_layer_order_from_compact_pair_assignment_with_observer_v2<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    input: GlobalFlatLayerOrderCompactPairAssignmentInputV2<'_>,
    limits: GlobalFlatLayerOrderCompactPairAssignmentLimitsV2,
    observer: &mut O,
) -> Result<
    GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    GlobalFlatLayerOrderCompactPairAssignmentErrorV2,
> {
    if !compact_pair_assignment_limits_are_finite_v2(limits) {
        return Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::InvalidLimits);
    }
    let assignment_bytes = input.direction_bits_le.len();
    if assignment_bytes > limits.max_compact_assignment_bytes {
        return Err(compact_resource_error_v2(
            FlatFoldabilityResource::CompactPairAssignmentBytes,
            limits.max_compact_assignment_bytes,
            assignment_bytes,
        ));
    }
    if input.variable_count > limits.analysis.max_overlap_face_pairs {
        return Err(compact_resource_error_v2(
            FlatFoldabilityResource::OverlapFacePairs,
            limits.analysis.max_overlap_face_pairs,
            input.variable_count,
        ));
    }
    let expected_assignment_bytes = facewise::compact_assignment_byte_len_v2(input.variable_count)
        .ok_or(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
            GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::ByteLength {
                expected: usize::MAX,
                actual: assignment_bytes,
            },
        ))?;
    if assignment_bytes != expected_assignment_bytes {
        return Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
            GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::ByteLength {
                expected: expected_assignment_bytes,
                actual: assignment_bytes,
            },
        ));
    }
    if facewise::compact_assignment_has_nonzero_tail_v2(
        input.direction_bits_le,
        input.variable_count,
    ) {
        return Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
            GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::NonZeroTailBits,
        ));
    }
    if assignment_bytes > limits.max_peak_bytes {
        return Err(compact_resource_error_v2(
            FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
            limits.max_peak_bytes,
            assignment_bytes,
        ));
    }

    let mut validation_peak =
        LiveValidationPeakLedgerV2::new(assignment_bytes, limits.max_peak_bytes);
    let validated = match validate_global_flat_source_with_observer(
        input.source,
        limits.analysis,
        None,
        Some(&mut validation_peak),
        observer,
    ) {
        Ok(validated) => validated,
        Err(failure) => {
            return Err(match *failure {
                GlobalFlatSourceValidationFailure::Unknown { reason, .. } => {
                    GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
                        reason: compact_unknown_reason_v2(reason),
                    }
                }
                GlobalFlatSourceValidationFailure::Impossible { .. } => {
                    GlobalFlatLayerOrderCompactPairAssignmentErrorV2::LiveSourceImpossible
                }
                GlobalFlatSourceValidationFailure::Execution(error) => {
                    GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Execution(error)
                }
            });
        }
    };
    let canonical_face_bytes = validated
        .canonical_faces
        .capacity()
        .checked_mul(std::mem::size_of::<LayerFace>())
        .ok_or_else(|| {
            compact_resource_error_v2(
                FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
                limits.max_peak_bytes,
                usize::MAX,
            )
        })?;
    let borrowed_live_bytes = assignment_bytes
        .checked_add(canonical_face_bytes)
        .ok_or_else(|| {
            compact_resource_error_v2(
                FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
                limits.max_peak_bytes,
                usize::MAX,
            )
        })?;
    if borrowed_live_bytes > limits.max_peak_bytes {
        return Err(compact_resource_error_v2(
            FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
            limits.max_peak_bytes,
            borrowed_live_bytes,
        ));
    }
    let provenance = validated.provenance;
    let facewise = facewise::reconstruct_layer_order_from_compact_pair_assignment_v2(
        facewise::FacewiseCompactPairAssignmentInputV2 {
            paper: validated.paper,
            crease_pattern: validated.crease_pattern,
            topology: validated.topology,
            canonical_faces: &validated.canonical_faces,
            provenance,
            work_counts: validated.work_counts,
            limits: limits.analysis,
            variable_count: input.variable_count,
            variable_registry_sha256: input.variable_registry_sha256,
            direction_bits_le: input.direction_bits_le,
            borrowed_live_bytes,
            max_peak_bytes: limits.max_peak_bytes,
        },
        observer,
    )
    .map_err(map_facewise_compact_failure_v2)?;
    let facewise::FacewiseCompactPairAssignmentSuccessV2 {
        layer_order,
        work_counts,
        observed_peak_bytes: observed_facewise_peak_bytes,
    } = facewise;
    drop(validated);

    let mut retained_checkpoint = || match observer.checkpoint() {
        GlobalFlatFoldabilityCheckpoint::Continue => Ok(()),
        GlobalFlatFoldabilityCheckpoint::DeadlineReached => Err(
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::VerifyingCertificate,
                },
            },
        ),
        GlobalFlatFoldabilityCheckpoint::Cancelled => {
            Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled,
            ))
        }
    };
    let layer_order_retained_bytes = layer_order
        .checked_deep_retained_bytes_with_checkpoint_v2(&mut retained_checkpoint)?
        .ok_or_else(|| {
            compact_resource_error_v2(
                FlatFoldabilityResource::LayerOrderResultBytes,
                limits.max_layer_order_retained_bytes,
                usize::MAX,
            )
        })?;
    if layer_order_retained_bytes > limits.max_layer_order_retained_bytes {
        return Err(compact_resource_error_v2(
            FlatFoldabilityResource::LayerOrderResultBytes,
            limits.max_layer_order_retained_bytes,
            layer_order_retained_bytes,
        ));
    }
    let terminal_live_bytes = assignment_bytes
        .checked_add(layer_order_retained_bytes)
        .ok_or_else(|| {
            compact_resource_error_v2(
                FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
                limits.max_peak_bytes,
                usize::MAX,
            )
        })?;
    let observed_validation_peak_bytes = validation_peak.observed_peak_bytes;
    let observed_peak_bytes = observed_facewise_peak_bytes
        .max(observed_validation_peak_bytes)
        .max(terminal_live_bytes);
    if observed_peak_bytes > limits.max_peak_bytes {
        return Err(compact_resource_error_v2(
            FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
            limits.max_peak_bytes,
            observed_peak_bytes,
        ));
    }
    if work_counts.search_nodes != 0 || !layer_order.is_current_for(&provenance) {
        return Err(GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Execution(
            GlobalFlatFoldabilityExecutionError::Internal {
                reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
            },
        ));
    }
    Ok(GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2 {
        layer_order,
        provenance,
        work_counts,
        variable_count: input.variable_count,
        variable_registry_sha256: input.variable_registry_sha256,
        limits,
        resources: GlobalFlatLayerOrderCompactPairAssignmentResourcesV2 {
            compact_assignment_bytes: assignment_bytes,
            borrowed_live_bytes,
            layer_order_retained_bytes,
            observed_validation_peak_bytes,
            observed_facewise_peak_bytes,
            observed_peak_bytes,
        },
        _authority_seal: GlobalFlatLayerOrderCompactPairAssignmentSealV2,
    })
}

fn compact_pair_assignment_limits_are_finite_v2(
    limits: GlobalFlatLayerOrderCompactPairAssignmentLimitsV2,
) -> bool {
    let analysis = limits.analysis;
    ![
        analysis.max_source_vertices,
        analysis.max_source_edges,
        analysis.max_paper_boundary_vertices,
        analysis.max_faces,
        analysis.max_face_boundary_half_edges,
        analysis.max_hinges,
        analysis.max_edge_incidence_records,
        analysis.max_local_vertices,
        analysis.max_total_records,
        analysis.max_overlap_face_pairs,
        analysis.max_arrangement_segments,
        analysis.max_overlap_cells,
        analysis.max_constraints,
        analysis.max_search_nodes,
        analysis.max_exact_integer_bits,
        analysis.max_exact_operations,
        analysis.max_certificate_bytes,
        limits.max_compact_assignment_bytes,
        limits.max_layer_order_retained_bytes,
        limits.max_peak_bytes,
    ]
    .contains(&usize::MAX)
}

fn compact_resource_error_v2(
    resource: FlatFoldabilityResource,
    limit: usize,
    observed: usize,
) -> GlobalFlatLayerOrderCompactPairAssignmentErrorV2 {
    GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
        reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
            resource,
            limit,
            observed,
        },
    }
}

fn map_facewise_compact_failure_v2(
    failure: facewise::FacewiseCompactPairAssignmentFailureV2,
) -> GlobalFlatLayerOrderCompactPairAssignmentErrorV2 {
    match failure {
        facewise::FacewiseCompactPairAssignmentFailureV2::Inconclusive(reason) => {
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Inconclusive {
                reason: compact_unknown_reason_v2(reason),
            }
        }
        facewise::FacewiseCompactPairAssignmentFailureV2::LiveSourceImpossible => {
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::LiveSourceImpossible
        }
        facewise::FacewiseCompactPairAssignmentFailureV2::RegistryMismatch => {
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::RegistryMismatch
        }
        facewise::FacewiseCompactPairAssignmentFailureV2::MalformedAssignment => {
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Malformed(
                GlobalFlatLayerOrderCompactPairAssignmentMalformedV2::NonZeroTailBits,
            )
        }
        facewise::FacewiseCompactPairAssignmentFailureV2::AssignmentRejected => {
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::AssignmentRejected
        }
        facewise::FacewiseCompactPairAssignmentFailureV2::Execution(error) => {
            GlobalFlatLayerOrderCompactPairAssignmentErrorV2::Execution(error)
        }
    }
}

fn compact_unknown_reason_v2(
    reason: GlobalFlatFoldabilityUnknownReason,
) -> GlobalFlatFoldabilityUnknownReason {
    match reason {
        GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
            resource: FlatFoldabilityResource::LayerOrderRevalidationPeakBytes,
            limit,
            observed,
        } => GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
            resource: FlatFoldabilityResource::LayerOrderReconstructionPeakBytes,
            limit,
            observed,
        },
        other => other,
    }
}
