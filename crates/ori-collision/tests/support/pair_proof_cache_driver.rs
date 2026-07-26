use std::time::{Duration, Instant};

use ori_collision::{
    PersistentPairProofCacheRuntimeV1, ProofCacheOperationControlV1, ProofCacheRuntimeBindingV1,
    ProofCacheRuntimeCaptureV1, StackedFoldBoundedPathDiagnosticV1,
    StackedFoldPathDiagnosticErrorV1, StackedFoldPathDiagnosticLimitsV1,
    diagnose_collective_hinge_path_v1, diagnose_collective_hinge_path_with_pair_cache_v1,
};
use ori_domain::ProjectId;

use crate::support::{PAPER_THICKNESS_MM, ProductionTargetV1};

pub(crate) fn binding(
    instance: ProjectId,
    project: ProjectId,
    target: &ProductionTargetV1,
    pose_generation: u64,
) -> ProofCacheRuntimeBindingV1 {
    ProofCacheRuntimeBindingV1::new(
        instance,
        project,
        target.revision,
        target.fingerprint,
        pose_generation,
        PAPER_THICKNESS_MM,
    )
    .expect("cache binding")
}

pub(crate) fn live_control() -> ProofCacheOperationControlV1<'static> {
    ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(60))
}

pub(crate) fn uncached(
    target: &ProductionTargetV1,
    requested_angle_degrees: f64,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    diagnose_collective_hinge_path_v1(
        target.model(),
        target.pose(),
        &target.moving_hinges(),
        requested_angle_degrees,
        PAPER_THICKNESS_MM,
        StackedFoldPathDiagnosticLimitsV1 {
            sample_intervals: 1,
            ..StackedFoldPathDiagnosticLimitsV1::default()
        },
    )
}

pub(crate) fn cached(
    target: &ProductionTargetV1,
    requested_angle_degrees: f64,
    paper_thickness_mm: f64,
    runtime: &PersistentPairProofCacheRuntimeV1,
    capture: &ProofCacheRuntimeCaptureV1,
) -> Result<StackedFoldBoundedPathDiagnosticV1, StackedFoldPathDiagnosticErrorV1> {
    diagnose_collective_hinge_path_with_pair_cache_v1(
        target.model(),
        target.pose(),
        &target.moving_hinges(),
        requested_angle_degrees,
        paper_thickness_mm,
        StackedFoldPathDiagnosticLimitsV1 {
            sample_intervals: 1,
            ..StackedFoldPathDiagnosticLimitsV1::default()
        },
        runtime,
        capture,
        live_control(),
    )
}
