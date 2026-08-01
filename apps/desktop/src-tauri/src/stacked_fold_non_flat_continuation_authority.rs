use super::*;

pub(super) fn authority_revalidates_v1(
    authority: &NonFlatCycleContinuationAuthorityV1,
    project_id: ProjectId,
    revision: u64,
    source_fingerprint: [u8; 32],
    pose_generation: u64,
) -> bool {
    let paper_thickness_mm = f64::from_bits(authority.paper_thickness_bits);
    if authority.pose_capability.generation() != pose_generation
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
    {
        return false;
    }
    let Some((tree_model, tree_pose)) = authority.pose_capability.tree() else {
        return false;
    };
    let Some((geometry, audit, graph_pose)) = authority.pose_capability.graph() else {
        return false;
    };
    let schedule = authority.generated.schedule();
    let Some(source_angles) = schedule.evaluate(0.0) else {
        return false;
    };
    let Some(target_angles) = schedule.evaluate(1.0) else {
        return false;
    };
    let source_pose = pose_state_fingerprint_v1(&source_angles);
    let target_pose = pose_state_fingerprint_v1(&target_angles);
    let moving_hinges = source_angles
        .as_slice()
        .iter()
        .zip(target_angles.as_slice())
        .filter_map(|(source, target)| {
            (source.edge() == target.edge()
                && source.angle_degrees().to_bits() != target.angle_degrees().to_bits())
            .then_some(source.edge())
        })
        .collect::<Vec<_>>();
    exact_hinge_angles_match_v1(source_angles.as_slice(), tree_pose.hinge_angles())
        && exact_hinge_angles_match_v1(
            source_angles.as_slice(),
            graph_pose.hinge_angles().as_slice(),
        )
        && exact_hinge_angles_match_v1(source_angles.as_slice(), authority.source.hinge_angles())
        && exact_hinge_angles_match_v1(target_angles.as_slice(), authority.target.hinge_angles())
        && moving_hinges.as_slice() == authority.generated.moving_hinges()
        && authority.source.identity_namespace() == project_id
        && authority.source.target_revision() == revision
        && authority.source.target_fingerprint().0 == source_fingerprint
        && authority.source.fixed_face() == tree_pose.fixed_face()
        && authority.source.fixed_face() == Some(graph_pose.fixed_face())
        && authority.target.identity_namespace() == project_id
        && authority.target.target_revision() == revision.saturating_add(1)
        && authority.target.target_fingerprint().0 == source_fingerprint
        && authority.target.fixed_face() == authority.source.fixed_face()
        && authority
            .positive
            .is_for(tree_model, tree_pose, &target_angles, paper_thickness_mm)
        && authority.closure.fixed_face() == graph_pose.fixed_face()
        && authority.closure.every_leaf_covers_graph_v1(geometry)
        && authority.closure.schedule_binding_fingerprint_v2()
            == schedule.certificate_binding_fingerprint_v2()
        && authority.closure.graph_binding_fingerprint_v1()
            == schedule.graph_binding_fingerprint_v1()
        && schedule.matches_binding(geometry, audit, graph_pose.fixed_face())
        && authority
            .transport
            .is_for(&authority.source, &authority.target)
        && authority.path.source() == source_pose
        && authority.path.target() == target_pose
        && authority.path.edges().len() == 1
        && authority.path.edges()[0].source() == source_pose
        && authority.path.edges()[0].target() == target_pose
        && authority.path.edges()[0].schedule_certificate()
            == schedule.certificate_binding_fingerprint_v2()
        && authority.path.edges()[0].collision_certificate()
            == authority.positive.binding_fingerprint_v1()
        && authority.path.edges()[0].closure_certificate()
            == authority.closure.partition_binding_fingerprint_v2()
        && authority.binding
            == non_flat_cycle_authority_binding_v1(
                project_id,
                revision,
                source_fingerprint,
                pose_generation,
                schedule,
                &authority.closure,
                &authority.positive,
                &authority.source,
                &authority.target,
                &authority.path,
                paper_thickness_mm,
            )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn non_flat_cycle_authority_binding_v1(
    project_id: ProjectId,
    revision: u64,
    source_fingerprint: [u8; 32],
    pose_generation: u64,
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    positive: &PositiveThicknessTreeContinuousCertificateV1,
    source: &StackedFoldNonFlatLayerOrderV1,
    target: &StackedFoldNonFlatLayerOrderV1,
    path: &ori_collision::CertifiedPoseGraphPathCertificateV1,
    paper_thickness_mm: f64,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(NON_FLAT_CYCLE_CONTINUATION_MODEL_ID_V1.as_bytes());
    hash.update(project_id.canonical_bytes());
    hash.update(revision.to_be_bytes());
    hash.update(source_fingerprint);
    hash.update(pose_generation.to_be_bytes());
    hash.update(schedule.certificate_binding_fingerprint_v2());
    hash.update(closure.partition_binding_fingerprint_v2());
    hash.update(positive.binding_fingerprint_v1());
    hash.update(path.binding_fingerprint_v1());
    hash.update(paper_thickness_mm.to_bits().to_be_bytes());
    hash_non_flat_layer_order_v1(&mut hash, source);
    hash_non_flat_layer_order_v1(&mut hash, target);
    hash.finalize().into()
}

fn hash_non_flat_layer_order_v1(hash: &mut Sha256, value: &StackedFoldNonFlatLayerOrderV1) {
    hash.update(value.identity_namespace().canonical_bytes());
    hash.update(value.target_revision().to_be_bytes());
    hash.update(value.target_fingerprint().0);
    match value.fixed_face() {
        Some(face) => {
            hash.update([1]);
            hash.update(face.canonical_bytes());
        }
        None => hash.update([0]),
    }
    hash.update((value.hinge_angles().len() as u64).to_be_bytes());
    for angle in value.hinge_angles() {
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_be_bytes());
    }
    hash.update((value.material_faces().len() as u64).to_be_bytes());
    for face in value.material_faces() {
        hash.update(face.face_id.canonical_bytes());
        hash.update(face.face_key.0);
    }
    hash.update((value.folded_faces().len() as u64).to_be_bytes());
    for folded in value.folded_faces() {
        hash.update(folded.face().face_id.canonical_bytes());
        hash.update(folded.face().face_key.0);
        hash.update([folded.dropped_world_axis()]);
        hash_exact_transform_v1(hash, folded.source_to_plane());
    }
    hash.update((value.tested_face_pairs() as u64).to_be_bytes());
    hash.update((value.source_overlap_cells_authenticated() as u64).to_be_bytes());
    hash.update((value.overlap_cells().len() as u64).to_be_bytes());
    for cell in value.overlap_cells() {
        hash.update(cell.lower_face().canonical_bytes());
        hash.update(cell.upper_face().canonical_bytes());
        hash.update((cell.boundary().len() as u64).to_be_bytes());
        for point in cell.boundary() {
            hash.update(point.x.to_bits().to_be_bytes());
            hash.update(point.y.to_bits().to_be_bytes());
        }
        hash.update((cell.exact_boundary().len() as u64).to_be_bytes());
        for point in cell.exact_boundary() {
            hash_exact_rational_v1(hash, &point.x);
            hash_exact_rational_v1(hash, &point.y);
        }
    }
    hash.update((value.face_pair_orders().len() as u64).to_be_bytes());
    for pair in value.face_pair_orders() {
        hash.update(pair.lower_face().canonical_bytes());
        hash.update(pair.upper_face().canonical_bytes());
    }
}

fn hash_exact_transform_v1(hash: &mut Sha256, value: &ExactAffineTransform) {
    for coefficient in [
        &value.m00, &value.m01, &value.m10, &value.m11, &value.tx, &value.ty,
    ] {
        hash_exact_rational_v1(hash, coefficient);
    }
}

fn hash_exact_rational_v1(hash: &mut Sha256, value: &ExactRationalValue) {
    hash.update([match value.sign {
        ExactSign::Negative => 0,
        ExactSign::Zero => 1,
        ExactSign::Positive => 2,
    }]);
    hash.update((value.numerator_magnitude_be.len() as u64).to_be_bytes());
    hash.update(&value.numerator_magnitude_be);
    hash.update((value.denominator_be.len() as u64).to_be_bytes());
    hash.update(&value.denominator_be);
}

pub(super) fn freshly_analyze_flat_layer_order_v1(
    project_id: ProjectId,
    revision: u64,
    pattern: &ori_domain::CreasePattern,
    paper: &ori_domain::Paper,
) -> Result<LayerOrderSnapshot, String> {
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id,
        source_revision: revision,
        paper,
        pattern,
    })
    .snapshot
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let local = analyze_local_flat_foldability(paper, pattern);
    let report = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            project_id, paper, pattern, &topology, &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    match report.outcome {
        GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } => Ok(*layer_order),
        GlobalFlatFoldabilityOutcome::Impossible { .. }
        | GlobalFlatFoldabilityOutcome::Unknown { .. } => {
            Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
        }
    }
}

pub(super) fn canonical_target_angles_v1(
    input: &[NonFlatCycleContinuationAngleV1],
) -> Result<CanonicalHingeAngles, String> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(input.len())
        .map_err(|_| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    for entry in input {
        if entry.angle_degrees.to_bits() == (-0.0_f64).to_bits() {
            return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
        }
        values.push(
            HingeAngle::new(entry.edge, entry.angle_degrees)
                .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?,
        );
    }
    values.sort_unstable_by_key(|value| value.edge().canonical_bytes());
    CanonicalHingeAngles::new(values).map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())
}

pub(super) fn exact_hinge_angles_match_v1(left: &[HingeAngle], right: &[HingeAngle]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.edge() == right.edge()
                && left.angle_degrees().to_bits() == right.angle_degrees().to_bits()
        })
}

pub(super) fn map_non_flat_layer_error_v1(
    error: ori_core::PrepareStackedFoldNonFlatLayerOrderErrorV1,
) -> String {
    match error {
        ori_core::PrepareStackedFoldNonFlatLayerOrderErrorV1::ResourceLimit => {
            CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
        }
        _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
    }
}

pub(super) fn map_non_flat_transport_error_v1(
    error: ori_collision::NonFlatCellTransportErrorV1,
) -> String {
    match error {
        ori_collision::NonFlatCellTransportErrorV1::ResourceLimit => {
            CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
        }
        _ => CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned(),
    }
}

pub(super) fn lowercase_hex_v1(value: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in value {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

pub(super) fn parse_sha256_v1(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = std::str::from_utf8(pair)
            .ok()
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    }
    Ok(output)
}
