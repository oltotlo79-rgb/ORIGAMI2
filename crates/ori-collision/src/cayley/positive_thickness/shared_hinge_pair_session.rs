//! One exact-parent session for bounded pair-local shared-hinge diagnostics.
//!
//! The session owns one whole exact tree pose and lends it to one edge
//! diagnostic at a time. It does not aggregate pair results and cannot mint a
//! whole-pose collision or mutation authority.

use super::projected_pair_authority::{
    ProjectedPairAuthorityLimitsV1, projected_tree_counts_fit_limits_v1,
};
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SharedHingePairDiagnosticSessionErrorV1 {
    Diagnostic(SharedHingeSolidDiagnosticErrorV1),
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug)]
pub(crate) struct SharedHingePairDiagnosticSessionV1<'pose> {
    bound: BoundMaterialTreePose<'pose>,
    exact: RationalCayleyTreePose<'pose>,
    paper_thickness_bits: u64,
    full_face_count: usize,
    full_hinge_count: usize,
    excluded_face_index_bindings: usize,
    limits: ProjectedPairAuthorityLimitsV1,
}

impl<'pose> SharedHingePairDiagnosticSessionV1<'pose> {
    pub(crate) fn diagnose(
        &self,
        target_edge: Option<EdgeId>,
    ) -> Result<Option<SharedHingeSolidDiagnosticSummaryV1>, SharedHingeSolidDiagnosticErrorV1>
    {
        match self
            .diagnose_with_control_v1(target_edge, &CooperativeOperationControlV1::unbounded())
        {
            Ok(summary) => Ok(summary),
            Err(SharedHingePairDiagnosticSessionErrorV1::Diagnostic(error)) => Err(error),
            Err(
                SharedHingePairDiagnosticSessionErrorV1::Cancelled
                | SharedHingePairDiagnosticSessionErrorV1::DeadlineExceeded,
            ) => unreachable!("unbounded cooperative control cannot stop"),
        }
    }

    pub(crate) fn diagnose_with_control_v1(
        &self,
        target_edge: Option<EdgeId>,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<Option<SharedHingeSolidDiagnosticSummaryV1>, SharedHingePairDiagnosticSessionErrorV1>
    {
        shared_hinge_session_checkpoint_v1(control)?;
        let paper_thickness_mm = f64::from_bits(self.paper_thickness_bits);
        if !self.revalidates_for(self.bound, paper_thickness_mm) {
            return Err(SharedHingePairDiagnosticSessionErrorV1::Diagnostic(
                SharedHingeSolidDiagnosticErrorV1::InconsistentPose,
            ));
        }
        let diagnostic = diagnose_bound_shared_hinge_solid_from_exact_for_edge_with_control_v1(
            &self.exact,
            self.bound,
            paper_thickness_mm,
            target_edge,
            control,
        )
        .map_err(SharedHingePairDiagnosticSessionErrorV1::Diagnostic)?;
        shared_hinge_session_checkpoint_v1(control)?;
        Ok(diagnostic)
    }

    pub(crate) fn revalidates_for(
        &self,
        bound: BoundMaterialTreePose<'_>,
        paper_thickness_mm: f64,
    ) -> bool {
        positive_finite_binary64(paper_thickness_mm)
            && self.paper_thickness_bits == paper_thickness_mm.to_bits()
            && std::ptr::eq(self.bound.model(), bound.model())
            && std::ptr::eq(self.bound.pose(), bound.pose())
            && self.exact.is_for(bound)
            && self.full_face_count == self.exact.faces.len()
            && self.full_face_count == bound.model().face_ids().len()
            && self.full_hinge_count == self.exact.hinges.len()
            && self.full_hinge_count == bound.model().hinges().len()
            && self.full_hinge_count == bound.pose().hinge_angles().len()
            && bound.model().face_ids() == bound.pose().face_ids()
            && bound.model().hinges() == bound.pose().hinges()
            && projected_tree_counts_fit_limits_v1(
                self.full_face_count,
                self.full_hinge_count,
                self.limits,
            ) == Some(self.excluded_face_index_bindings)
    }

    #[cfg(test)]
    pub(super) const fn full_face_count_for_test(&self) -> usize {
        self.full_face_count
    }

    #[cfg(test)]
    pub(super) const fn full_hinge_count_for_test(&self) -> usize {
        self.full_hinge_count
    }

    #[cfg(test)]
    pub(super) const fn excluded_face_index_bindings_for_test(&self) -> usize {
        self.excluded_face_index_bindings
    }

    #[cfg(test)]
    pub(super) fn exact_for_test(&self) -> &RationalCayleyTreePose<'pose> {
        &self.exact
    }
}

fn shared_hinge_session_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), SharedHingePairDiagnosticSessionErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => SharedHingePairDiagnosticSessionErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            SharedHingePairDiagnosticSessionErrorV1::DeadlineExceeded
        }
    })
}

pub(crate) fn prepare_shared_hinge_pair_diagnostic_session_v1<'pose>(
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
) -> Result<Option<SharedHingePairDiagnosticSessionV1<'pose>>, SharedHingeSolidDiagnosticErrorV1> {
    prepare_shared_hinge_pair_diagnostic_session_with_limits_v1(
        bound,
        paper_thickness_mm,
        ProjectedPairAuthorityLimitsV1::default(),
    )
}

pub(super) fn prepare_shared_hinge_pair_diagnostic_session_with_limits_v1<'pose>(
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: ProjectedPairAuthorityLimitsV1,
) -> Result<Option<SharedHingePairDiagnosticSessionV1<'pose>>, SharedHingeSolidDiagnosticErrorV1> {
    if !positive_finite_binary64(paper_thickness_mm)
        || bound.model().face_ids() != bound.pose().face_ids()
        || bound.model().hinges() != bound.pose().hinges()
    {
        return Ok(None);
    }
    let full_face_count = bound.model().face_ids().len();
    let full_hinge_count = bound.model().hinges().len();
    if full_face_count < 2 || full_hinge_count == 0 {
        return Ok(None);
    }
    if full_hinge_count.checked_add(1) != Some(full_face_count) {
        return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
    }
    let limits = limits.clamped_to_hard();
    let excluded_face_index_bindings =
        projected_tree_counts_fit_limits_v1(full_face_count, full_hinge_count, limits)
            .ok_or(SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    let exact = match prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default()) {
        Ok(exact) => exact,
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            return Err(SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded);
        }
        Err(CayleyError::InvariantFailure { .. } | CayleyError::BoundTreeInconsistent { .. }) => {
            return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
        }
        Err(_) => return Ok(None),
    };
    if exact.faces.len() != full_face_count
        || exact.hinges.len() != full_hinge_count
        || !exact.is_for(bound)
    {
        return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
    }
    Ok(Some(SharedHingePairDiagnosticSessionV1 {
        bound,
        exact,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        full_face_count,
        full_hinge_count,
        excluded_face_index_bindings,
        limits,
    }))
}
