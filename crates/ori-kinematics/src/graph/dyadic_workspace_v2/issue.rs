use super::*;

impl MaterialHingeGraphGeometry {
    /// Generic, allocation-bounded adaptive dyadic closure primitive.
    ///
    /// It intentionally knows nothing about common-articulation or N>=33
    /// admission. Higher-level wrappers may apply those structural policies to
    /// a geometry/audit/schedule before invoking this crate-private engine. The
    /// borrowed schedule's retained bytes are not part of this engine's peak;
    /// an owning/restriction wrapper must add them and restriction scratch.
    #[allow(dead_code)] // Phase 2 connects the general-N wrappers to this seam.
    pub(crate) fn prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        tolerance: f64,
        limits: DyadicIntervalClosureWorkspaceLimitsV2,
        mut checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<
        WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
        DyadicIntervalClosureControlErrorV1,
    > {
        closure_checkpoint_v1(&mut checkpoint)?;
        if !tolerance.is_finite()
            || tolerance < 0.0
            || limits.max_depth >= 64
            || audit.faces().is_empty()
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        if limits_contain_usize_max_v2(limits)
            || limits.max_leaves == 0
            || limits.max_work == 0
            || limits.max_theorem_recognizer_work == 0
            || limits.schedule_limits.max_hinges == 0
            || limits.schedule_limits.max_work == 0
            || limits.schedule_limits.max_coefficient_bits == u32::MAX
        {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        if self.face_ids().len() != audit.faces().len() {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        let audit_hinge_count = audit
            .spanning_hinges()
            .len()
            .checked_add(audit.closure_hinges().len())
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if self.hinges().is_empty() || self.hinges().len() != audit_hinge_count {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }

        let schedule_workspace_bound = schedule
            .checked_dyadic_workspace_upper_bound_with_checkpoint_v2(
                limits.max_depth,
                limits.schedule_limits,
                || {
                    checkpoint().map_err(|stop| match stop {
                        DyadicIntervalClosureStopV1::Cancelled => {
                            CycleScheduleDyadicEvaluationStopV2::Cancelled
                        }
                        DyadicIntervalClosureStopV1::DeadlineExceeded => {
                            CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
                        }
                    })
                },
            )
            .map_err(|error| match error {
                CycleScheduleDyadicEvaluationErrorV2::Cancelled => {
                    DyadicIntervalClosureControlErrorV1::Cancelled
                }
                CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded => {
                    DyadicIntervalClosureControlErrorV1::DeadlineExceeded
                }
                CycleScheduleDyadicEvaluationErrorV2::Prepare(_)
                | CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit => {
                    DyadicIntervalClosureErrorV1::ResourceLimit.into()
                }
            })?;
        let preflight = checked_preflight_v2(self, audit, schedule_workspace_bound, limits)
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let mut resources = preflight.resources;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }

        let binding_matches = schedule
            .matches_binding_with_checkpoint_v2(self, audit, fixed_face, &mut checkpoint)
            .map_err(|stop| match stop {
                DyadicIntervalClosureStopV1::Cancelled => {
                    DyadicIntervalClosureControlErrorV1::Cancelled
                }
                DyadicIntervalClosureStopV1::DeadlineExceeded => {
                    DyadicIntervalClosureControlErrorV1::DeadlineExceeded
                }
            })?;
        if !binding_matches {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        for (geometry_face, audit_face) in self.face_ids().iter().zip(audit.faces()) {
            closure_checkpoint_v1(&mut checkpoint)?;
            if geometry_face != audit_face {
                return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
            }
        }
        let mut fixed_face_present = false;
        for face in audit.faces() {
            closure_checkpoint_v1(&mut checkpoint)?;
            if *face == fixed_face {
                fixed_face_present = true;
                break;
            }
        }
        if !fixed_face_present || !validate_audit_order_with_checkpoint_v2(audit, &mut checkpoint)?
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }

        closure_checkpoint_v1(&mut checkpoint)?;
        let mut canonical_hinge_indices = Vec::<usize>::new();
        canonical_hinge_indices
            .try_reserve_exact(self.hinges().len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let physical_carrier_index_bytes =
            checked_vec_bytes_v2::<usize>(canonical_hinge_indices.capacity())
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_carrier_index_workspace_upper_bound_bytes = resources
            .charged_carrier_index_workspace_upper_bound_bytes
            .max(physical_carrier_index_bytes);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        for index in 0..self.hinges().len() {
            closure_checkpoint_v1(&mut checkpoint)?;
            canonical_hinge_indices.push(index);
        }
        checkpoint_heap_sort_by_key_v1(
            &mut canonical_hinge_indices,
            |index| self.hinges()[*index].edge().canonical_bytes(),
            &mut checkpoint,
        )
        .map_err(map_heap_sort_error_v2)?;
        let mut canonical_checked_hinges = Vec::<EdgeId>::new();
        canonical_checked_hinges
            .try_reserve_exact(self.hinges().len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let retained_after_carrier_reserve =
            size_of::<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2>()
                .checked_add(
                    checked_vec_bytes_v2::<(u32, u64)>(limits.max_leaves)
                        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?,
                )
                .and_then(|bytes| {
                    checked_vec_bytes_v2::<EdgeId>(canonical_checked_hinges.capacity())
                        .and_then(|carrier| bytes.checked_add(carrier))
                })
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_retained_material_upper_bound_bytes = resources
            .charged_retained_material_upper_bound_bytes
            .max(retained_after_carrier_reserve);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        for index in &canonical_hinge_indices {
            closure_checkpoint_v1(&mut checkpoint)?;
            canonical_checked_hinges.push(self.hinges()[*index].edge());
        }
        if !validate_carrier_with_checkpoint_v2(
            self,
            audit,
            &canonical_hinge_indices,
            &canonical_checked_hinges,
            &mut checkpoint,
        )? {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        let mut pending = Vec::<(u32, u64)>::new();
        pending
            .try_reserve_exact(limits.max_leaves)
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let physical_partition_workspace = checked_vec_bytes_v2::<(u32, u64)>(pending.capacity())
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_partition_workspace_upper_bound_bytes = resources
            .charged_partition_workspace_upper_bound_bytes
            .max(physical_partition_workspace);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        let mut partition = Vec::<(u32, u64)>::new();
        partition
            .try_reserve_exact(limits.max_leaves)
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let physical_retained = size_of::<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2>()
            .checked_add(
                checked_vec_bytes_v2::<(u32, u64)>(partition.capacity())
                    .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?,
            )
            .and_then(|bytes| {
                checked_vec_bytes_v2::<EdgeId>(canonical_checked_hinges.capacity())
                    .and_then(|carrier| bytes.checked_add(carrier))
            })
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_retained_material_upper_bound_bytes = resources
            .charged_retained_material_upper_bound_bytes
            .max(physical_retained);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }

        let theorem_live_base = resources
            .charged_carrier_index_workspace_upper_bound_bytes
            .checked_add(resources.charged_partition_workspace_upper_bound_bytes)
            .and_then(|bytes| {
                bytes.checked_add(resources.charged_retained_material_upper_bound_bytes)
            })
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let max_theorem_workspace_bytes = limits
            .max_peak_workspace_bytes
            .checked_sub(theorem_live_base)
            .map(|peak_remaining| peak_remaining.min(limits.max_theorem_recognizer_workspace_bytes))
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let recognition = recognize_exact_parallel_cut_with_checkpoint_v2(
            self,
            schedule,
            &canonical_hinge_indices,
            &canonical_checked_hinges,
            limits.max_theorem_recognizer_work,
            max_theorem_workspace_bytes,
            &mut checkpoint,
        )
        .map_err(map_interval_control_error_v2)?;
        let (exact_parallel_cut, theorem_work, theorem_workspace_bytes) = match recognition {
            ExactParallelCutRecognitionV2::Proven {
                charged_work,
                workspace_bytes,
            } => {
                closure_checkpoint_v1(&mut checkpoint)?;
                partition.push((0, 0));
                (true, charged_work, workspace_bytes)
            }
            ExactParallelCutRecognitionV2::NotApplicable {
                charged_work,
                workspace_bytes,
            } => (false, charged_work, workspace_bytes),
        };
        resources.charged_theorem_recognizer_work =
            resources.charged_theorem_recognizer_work.max(theorem_work);
        resources.charged_theorem_recognizer_upper_bound_bytes = resources
            .charged_theorem_recognizer_upper_bound_bytes
            .max(theorem_workspace_bytes);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        if !exact_parallel_cut {
            pending.push((0, 0));
        }
        let mut visited = usize::from(exact_parallel_cut);
        while let Some((depth, index)) = pending.pop() {
            closure_checkpoint_v1(&mut checkpoint)?;
            visited = visited
                .checked_add(1)
                .filter(|value| *value <= limits.max_work)
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;

            let evaluation = schedule.evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
                depth,
                index,
                limits.schedule_limits,
                preflight.schedule,
                preflight.schedule.peak_bytes(),
                || {
                    checkpoint().map_err(|stop| match stop {
                        DyadicIntervalClosureStopV1::Cancelled => {
                            CycleScheduleDyadicEvaluationStopV2::Cancelled
                        }
                        DyadicIntervalClosureStopV1::DeadlineExceeded => {
                            CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded
                        }
                    })
                },
            );
            let evaluation = match evaluation {
                Ok(evaluation) => evaluation,
                Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit) => {
                    return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
                }
                Err(CycleScheduleDyadicEvaluationErrorV2::Prepare(
                    crate::CycleSchedulePrepareErrorV1::ResourceLimit
                    | crate::CycleSchedulePrepareErrorV1::AngleRange,
                )) if depth < limits.max_depth => {
                    closure_checkpoint_v1(&mut checkpoint)?;
                    split_partition_leaf_v2(depth, index, &mut pending, partition.len(), limits)?;
                    continue;
                }
                Err(CycleScheduleDyadicEvaluationErrorV2::Prepare(
                    crate::CycleSchedulePrepareErrorV1::ResourceLimit
                    | crate::CycleSchedulePrepareErrorV1::AngleRange,
                )) => return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into()),
                Err(CycleScheduleDyadicEvaluationErrorV2::Prepare(_)) => {
                    return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
                }
                Err(CycleScheduleDyadicEvaluationErrorV2::Cancelled) => {
                    return Err(DyadicIntervalClosureControlErrorV1::Cancelled);
                }
                Err(CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded) => {
                    return Err(DyadicIntervalClosureControlErrorV1::DeadlineExceeded);
                }
            };
            let observed_exact_objects = evaluation
                .exact_vector_capacity_peak_bytes()
                .checked_add(preflight.schedule.exact_nonvector_object_bytes())
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            resources.charged_exact_rational_object_upper_bound_bytes = resources
                .charged_exact_rational_object_upper_bound_bytes
                .max(observed_exact_objects);
            let observed_schedule_peak = evaluation
                .angle_box_capacity_bytes()
                .checked_add(preflight.schedule.big_rational_payload_bytes())
                .and_then(|bytes| {
                    bytes.checked_add(resources.charged_exact_rational_object_upper_bound_bytes)
                })
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            resources.charged_schedule_evaluation_workspace_upper_bound_bytes = resources
                .charged_schedule_evaluation_workspace_upper_bound_bytes
                .max(observed_schedule_peak);
            refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            if !resources_fit_limits_v2(resources, limits) {
                return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
            }

            let interval = prove_interval_closure_with_workspace_v2(
                IntervalClosureRequestV2 {
                    geometry: self,
                    audit,
                    fixed_face,
                    canonical_hinge_indices: &canonical_hinge_indices,
                    angle_boxes: evaluation.angle_boxes(),
                    tolerance,
                    max_work: limits.max_work,
                    max_workspace_bytes: limits.max_interval_closure_workspace_bytes,
                    max_pose_capacity_bytes: limits.max_interval_closure_workspace_bytes,
                    verification_mode: IntervalClosureVerificationModeV2::FullClosure,
                },
                &mut checkpoint,
            );
            match interval {
                Ok(success) => {
                    resources.charged_interval_closure_workspace_upper_bound_bytes = resources
                        .charged_interval_closure_workspace_upper_bound_bytes
                        .max(success.physical_capacity_bytes);
                    refresh_peak_v2(&mut resources)
                        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
                    if !resources_fit_limits_v2(resources, limits)
                        || partition.len() >= limits.max_leaves
                    {
                        return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
                    }
                    closure_checkpoint_v1(&mut checkpoint)?;
                    partition.push((depth, index));
                }
                Err(IntervalAttemptErrorV2::Unproven) if depth < limits.max_depth => {
                    closure_checkpoint_v1(&mut checkpoint)?;
                    split_partition_leaf_v2(depth, index, &mut pending, partition.len(), limits)?;
                }
                Err(IntervalAttemptErrorV2::Unproven) => {
                    return Err(
                        DyadicIntervalClosureErrorV1::UnprovenClosure { depth, index }.into(),
                    );
                }
                Err(error) => return Err(map_interval_control_error_v2(error)),
            }
        }

        resources.visited_partition_nodes = visited;
        resources.issued_leaves = partition.len();
        if !validate_partition_with_checkpoint_v2(&partition, &mut checkpoint)?
            || !validate_carrier_with_checkpoint_v2(
                self,
                audit,
                &canonical_hinge_indices,
                &canonical_checked_hinges,
                &mut checkpoint,
            )?
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        let schedule_binding_fingerprint_v2 = schedule.certificate_binding_fingerprint_v2();
        let graph_binding_fingerprint_v1 = schedule.graph_binding_fingerprint_v1();
        let partition_binding_fingerprint_v2 = compute_partition_binding_with_checkpoint_v2(
            fixed_face,
            schedule_binding_fingerprint_v2,
            graph_binding_fingerprint_v1,
            tolerance.to_bits(),
            limits,
            &partition,
            &canonical_checked_hinges,
            resources,
            exact_parallel_cut,
            &mut checkpoint,
        )?;
        let material = WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2 {
            issuer_geometry: self.instance_anchor_v1(),
            fixed_face,
            schedule_binding_fingerprint_v2,
            graph_binding_fingerprint_v1,
            tolerance_bits: tolerance.to_bits(),
            policy: limits,
            partition,
            canonical_checked_hinges,
            resources,
            partition_binding_fingerprint_v2,
        };
        // Publication self-audit: consume every sealed field through the same
        // observation seam that the Phase 2 wrapper will use. No allocation or
        // exact/interval arithmetic occurs here.
        if !material.issuer_geometry.matches(self)
            || material.fixed_face != fixed_face
            || material.schedule_binding_fingerprint_v2 != schedule_binding_fingerprint_v2
            || material.graph_binding_fingerprint_v1 != graph_binding_fingerprint_v1
            || material.tolerance_bits != tolerance.to_bits()
            || material.policy != limits
            || material.resources() != resources
            || material.partition().len() != resources.issued_leaves
            || material.canonical_checked_hinges().len() != self.hinges().len()
            || material.partition_binding_fingerprint_v2() != partition_binding_fingerprint_v2
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        // Publication checkpoint: no allocation or fallible proof work occurs
        // between this poll and returning the sealed material.
        closure_checkpoint_v1(&mut checkpoint)?;
        Ok(material)
    }
}
