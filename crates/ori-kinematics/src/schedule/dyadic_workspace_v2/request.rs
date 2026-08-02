use super::*;

impl CanonicalCycleScheduleV1 {
    /// Allocation-free validation of an opaque dyadic workspace request.
    /// This seam lets a sealed downstream session reject foreign or
    /// inconsistent bounds before making any partition-coverage observation.
    pub(crate) fn validate_dyadic_workspace_request_with_checkpoint_v2(
        &self,
        depth: u32,
        limits: CycleScheduleLimitsV1,
        prevalidated_bound: CycleScheduleDyadicWorkspaceBoundV2,
        max_schedule_workspace_bytes: usize,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationStopV2>,
    ) -> Result<(), CycleScheduleDyadicEvaluationErrorV2> {
        self.validate_dyadic_workspace_request_impl_v2(
            depth,
            limits,
            prevalidated_bound,
            max_schedule_workspace_bytes,
            &mut || {
                checkpoint().map_err(|stop| match stop {
                    CycleScheduleDyadicEvaluationStopV2::Cancelled => {
                        CycleScheduleDyadicEvaluationErrorV2::Cancelled
                    }
                    CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded => {
                        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded
                    }
                })
            },
        )
    }

    pub(super) fn validate_dyadic_workspace_request_impl_v2(
        &self,
        depth: u32,
        limits: CycleScheduleLimitsV1,
        prevalidated_bound: CycleScheduleDyadicWorkspaceBoundV2,
        max_schedule_workspace_bytes: usize,
        checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
    ) -> Result<(), CycleScheduleDyadicEvaluationErrorV2> {
        checkpoint()?;
        if depth >= 64
            || depth > prevalidated_bound.max_depth
            || prevalidated_bound.limits != limits
            || prevalidated_bound.schedule_fingerprint_v2 != self.schedule_fingerprint_v2
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        if max_schedule_workspace_bytes == 0 || max_schedule_workspace_bytes == usize::MAX {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }
        let carrier_len = if self.half_angle_entries.is_empty() {
            self.entries.len()
        } else {
            self.half_angle_entries.len()
        };
        let logical_angle_bytes = std::mem::size_of::<(EdgeId, OutwardIntervalV1)>()
            .checked_mul(carrier_len)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        if logical_angle_bytes != prevalidated_bound.angle_box_bytes
            || prevalidated_bound
                .angle_box_bytes
                .checked_add(prevalidated_bound.big_rational_payload_bytes)
                .and_then(|bytes| bytes.checked_add(prevalidated_bound.exact_object_bytes))
                != Some(prevalidated_bound.peak_bytes)
            || prevalidated_bound.peak_bytes != max_schedule_workspace_bytes
        {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }
        checkpoint()?;
        Ok(())
    }

    /// Checkpointed retained-allocation inventory for callers that include a
    /// borrowed schedule in a larger replay peak. Every entry and nested exact
    /// coefficient is polled, including entry and prepublication polls.
    pub(crate) fn checked_deep_retained_bytes_with_checkpoint_v2(
        &self,
        maximum_retained_bytes: usize,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationStopV2>,
    ) -> Result<usize, CycleScheduleDyadicEvaluationErrorV2> {
        let mut poll = || {
            checkpoint().map_err(|stop| match stop {
                CycleScheduleDyadicEvaluationStopV2::Cancelled => {
                    CycleScheduleDyadicEvaluationErrorV2::Cancelled
                }
                CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded => {
                    CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded
                }
            })
        };
        poll()?;
        let mut total = std::mem::size_of::<Self>()
            .checked_add(
                std::mem::size_of::<Entry>()
                    .checked_mul(self.entries.capacity())
                    .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
            )
            .and_then(|value| {
                value.checked_add(
                    std::mem::size_of::<PreparedHalfAngleRationalEntryV1>()
                        .checked_mul(self.half_angle_entries.capacity())?,
                )
            })
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        if total > maximum_retained_bytes {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }
        for entry in &self.entries {
            poll()?;
            total = total
                .checked_add(
                    std::mem::size_of::<f64>()
                        .checked_mul(entry.coefficients.capacity())
                        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
                )
                .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
            if total > maximum_retained_bytes {
                return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
            }
            for _ in &entry.coefficients {
                poll()?;
            }
        }
        for entry in &self.half_angle_entries {
            poll()?;
            for value in &entry.u_domain {
                poll()?;
                total = total
                    .checked_add(
                        checked_big_rational_heap_bytes_upper_bound_v1(value)
                            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
                    )
                    .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
                if total > maximum_retained_bytes {
                    return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
                }
            }
            for coefficients in [
                &entry.numerator_power_coefficients,
                &entry.denominator_power_coefficients,
                &entry.numerator_certificate.coefficients,
                &entry.denominator_certificate.coefficients,
            ] {
                total = total
                    .checked_add(
                        std::mem::size_of::<BigRational>()
                            .checked_mul(coefficients.capacity())
                            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
                    )
                    .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
                if total > maximum_retained_bytes {
                    return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
                }
                for coefficient in coefficients {
                    poll()?;
                    total = total
                        .checked_add(
                            checked_big_rational_heap_bytes_upper_bound_v1(coefficient)
                                .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
                        )
                        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
                    if total > maximum_retained_bytes {
                        return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
                    }
                }
            }
        }
        poll()?;
        Ok(total)
    }

    /// Classifies the allocation-free ordinary profile admitted by the V2
    /// exact parallel-cut closure recognizer. Moving hinges must all carry one
    /// bit-identical affine `initial + c1*x` profile whose exact-real endpoint
    /// range lies strictly inside 0..180 degrees. Every other hinge must use
    /// the recognized zero encoding (`initial == c0 == 0`, no higher term).
    /// The caller edge slice is in canonical order.
    pub(crate) fn classify_exact_parallel_cut_profile_with_checkpoint_v2<Stop>(
        &self,
        canonical_edges: &[EdgeId],
        max_work: usize,
        mut checkpoint: impl FnMut() -> Result<(), Stop>,
    ) -> Result<
        (Option<ExactParallelCutScheduleProfileV2>, usize),
        ExactParallelCutProfileErrorV2<Stop>,
    > {
        let mut work = 0usize;
        exact_parallel_cut_profile_poll_and_charge_v2(&mut work, max_work, &mut checkpoint)?;
        if !self.half_angle_entries.is_empty() || self.entries.len() != canonical_edges.len() {
            return Ok((None, work));
        }
        let mut reference: Option<&Entry> = None;
        let mut moving_count = 0usize;
        for (entry, expected_edge) in self.entries.iter().zip(canonical_edges) {
            exact_parallel_cut_profile_poll_and_charge_v2(&mut work, max_work, &mut checkpoint)?;
            if entry.edge != *expected_edge || entry.coefficients.is_empty() {
                return Ok((None, work));
            }
            let mut constant = true;
            for coefficient in entry.coefficients.iter().skip(1) {
                exact_parallel_cut_profile_poll_and_charge_v2(
                    &mut work,
                    max_work,
                    &mut checkpoint,
                )?;
                constant &= *coefficient == 0.0;
            }
            if constant {
                if entry.initial != 0.0 || entry.coefficients[0] != 0.0 {
                    return Ok((None, work));
                }
                continue;
            }
            // The theorem deliberately admits only affine schedules. A
            // correctly-rounded endpoint strictly inside the representable
            // bounds implies the exact binary-rational sum is inside them too.
            if entry.coefficients.len() != 2 || entry.coefficients[0] != 0.0 {
                return Ok((None, work));
            }
            let slope = entry.coefficients[1];
            let lower = entry.initial - slope.abs();
            let upper = entry.initial + slope.abs();
            if !lower.is_finite() || !upper.is_finite() || lower <= 0.0 || upper >= 180.0 {
                return Ok((None, work));
            }
            if let Some(reference) = reference {
                if entry.initial.to_bits() != reference.initial.to_bits()
                    || entry.coefficients.len() != reference.coefficients.len()
                {
                    return Ok((None, work));
                }
                for (actual, expected) in entry.coefficients.iter().zip(&reference.coefficients) {
                    exact_parallel_cut_profile_poll_and_charge_v2(
                        &mut work,
                        max_work,
                        &mut checkpoint,
                    )?;
                    if actual.to_bits() != expected.to_bits() {
                        return Ok((None, work));
                    }
                }
            } else {
                reference = Some(entry);
            }
            moving_count = moving_count.saturating_add(1);
        }
        exact_parallel_cut_profile_poll_and_charge_v2(&mut work, max_work, &mut checkpoint)?;
        Ok((
            reference.map(|_| ExactParallelCutScheduleProfileV2 {
                schedule_fingerprint_v2: self.schedule_fingerprint_v2,
                moving_count,
                charged_work: work,
            }),
            work,
        ))
    }

    pub(crate) fn exact_parallel_cut_position_is_moving_v2(
        &self,
        profile: ExactParallelCutScheduleProfileV2,
        position: usize,
    ) -> Option<bool> {
        if profile.schedule_fingerprint_v2 != self.schedule_fingerprint_v2
            || profile.moving_count == 0
        {
            return None;
        }
        let entry = self.entries.get(position)?;
        Some(
            entry
                .coefficients
                .get(1)
                .is_some_and(|coefficient| *coefficient != 0.0),
        )
    }

    pub(crate) const fn exact_parallel_cut_profile_charged_work_v2(
        profile: ExactParallelCutScheduleProfileV2,
    ) -> usize {
        profile.charged_work
    }

    /// Allocation-free binding match with caller-defined cooperative stops.
    /// Every face, audit edge and material hinge is polled before hashing.
    pub(crate) fn matches_binding_with_checkpoint_v2<Stop>(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        checkpoint: &mut impl FnMut() -> Result<(), Stop>,
    ) -> Result<bool, Stop> {
        checkpoint()?;
        if self.fixed_face != fixed_face {
            return Ok(false);
        }
        let mut hash = Sha256::new();
        hash.update(fixed_face.canonical_bytes());
        for face in audit.faces() {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
        for edge in audit.spanning_hinges().iter().chain(audit.closure_hinges()) {
            checkpoint()?;
            hash.update(edge.canonical_bytes());
        }
        for hinge in geometry.hinges() {
            checkpoint()?;
            hash.update(hinge.edge().canonical_bytes());
            hash.update(hinge.left_face().canonical_bytes());
            hash.update(hinge.right_face().canonical_bytes());
            hash.update([match hinge.assignment() {
                ori_topology::FoldAssignment::Mountain => 0,
                ori_topology::FoldAssignment::Valley => 1,
            }]);
            for value in [
                hinge.start().x(),
                hinge.start().y(),
                hinge.start().z(),
                hinge.end().x(),
                hinge.end().y(),
                hinge.end().z(),
                hinge.axis().x(),
                hinge.axis().y(),
                hinge.axis().z(),
            ] {
                hash.update(value.to_bits().to_be_bytes());
            }
        }
        checkpoint()?;
        let binding: [u8; 32] = hash.finalize().into();
        Ok(self.binding_fingerprint == binding)
    }
}
