//! Opaque binding of one continuous cycle proof to exact layer-order witnesses.

use std::collections::{HashMap, HashSet};

use ori_domain::FaceId;
use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{
    CanonicalCycleScheduleV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CONTINUOUS_LAYER_TRANSPORT_CERTIFICATE_MODEL_ID_V1: &str =
    "native_continuous_layer_transport_certificate_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuousLayerTransportLimitsV1 {
    pub max_transitions: usize,
    pub max_pair_orders: usize,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ContinuousLayerTransportErrorV1 {
    #[error("continuous layer transport input is stale or issuer-mismatched")]
    BindingMismatch,
    #[error("continuous layer transport exceeds its resource limit")]
    ResourceLimit,
    #[error("a transition layer order is missing, duplicated, or ambiguous")]
    AmbiguousOrder,
    #[error("a face pair crosses or reverses its authenticated source order")]
    Crossing,
    #[error("a transition contains a colliding self-order witness")]
    Collision,
}

/// Performs the allocation-free checkpoint used before retaining transition
/// witnesses. Callers can therefore cancel even a maximum-size graph without
/// constructing or hashing any layer-order payload.
pub fn preflight_continuous_layer_transport_work_v1(
    transition_count: usize,
    pair_order_count: usize,
    limits: ContinuousLayerTransportLimitsV1,
) -> Result<usize, ContinuousLayerTransportErrorV1> {
    if transition_count == 0 || transition_count > limits.max_transitions {
        return Err(ContinuousLayerTransportErrorV1::ResourceLimit);
    }
    let work = transition_count
        .checked_mul(pair_order_count)
        .ok_or(ContinuousLayerTransportErrorV1::ResourceLimit)?;
    (work <= limits.max_pair_orders)
        .then_some(work)
        .ok_or(ContinuousLayerTransportErrorV1::ResourceLimit)
}

#[derive(Debug, Clone)]
pub struct ContinuousLayerTransportCertificateV1 {
    issuer: MaterialHingeGraphGeometry,
    source_instance: usize,
    source_hash: [u8; 32],
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    transition_hashes: Vec<[u8; 32]>,
    target_order_hash: [u8; 32],
    pair_order_count: usize,
}

impl ContinuousLayerTransportCertificateV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CONTINUOUS_LAYER_TRANSPORT_CERTIFICATE_MODEL_ID_V1
    }
    #[must_use]
    pub fn transition_hashes(&self) -> &[[u8; 32]] {
        &self.transition_hashes
    }
    #[must_use]
    pub const fn target_order_hash(&self) -> [u8; 32] {
        self.target_order_hash
    }
    #[must_use]
    pub const fn pair_order_count(&self) -> usize {
        self.pair_order_count
    }
    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        source: &LayerOrderSnapshot,
        schedule: &CanonicalCycleScheduleV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    ) -> bool {
        self.issuer.same_instance(geometry)
            && self.source_instance == source as *const LayerOrderSnapshot as usize
            && self.source_hash == hash_source(source)
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v1()
            && self.closure_hash == closure.partition_binding_fingerprint_v1()
    }
    /// Revalidates a transported/cloned snapshot after an outer native
    /// capability has independently authenticated its instance/generation.
    /// Unlike `is_for`, this deliberately checks exact content rather than
    /// allocation identity so a pending transaction can retain an owned copy.
    #[must_use]
    pub fn matches_source_content_v1(&self, source: &LayerOrderSnapshot) -> bool {
        self.source_hash == hash_source(source)
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

/// Binds canonical bottom-to-top pair orders at every dyadic transition.
/// `source_to_target` is the lineage transport supplied by the native caller.
pub fn prove_continuous_layer_transport_v1(
    geometry: &MaterialHingeGraphGeometry,
    source: &LayerOrderSnapshot,
    source_to_target: &[(FaceId, FaceId)],
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    transition_orders: &[Vec<(FaceId, FaceId)>],
    limits: ContinuousLayerTransportLimitsV1,
) -> Result<ContinuousLayerTransportCertificateV1, ContinuousLayerTransportErrorV1> {
    if !closure.every_leaf_covers_graph_v1(geometry)
        || closure.schedule_binding_fingerprint_v1()
            != schedule.certificate_binding_fingerprint_v1()
        || closure.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
    {
        return Err(ContinuousLayerTransportErrorV1::BindingMismatch);
    }
    let expected_transitions = closure
        .leaves()
        .len()
        .checked_add(1)
        .ok_or(ContinuousLayerTransportErrorV1::ResourceLimit)?;
    if transition_orders.len() != expected_transitions {
        return Err(ContinuousLayerTransportErrorV1::ResourceLimit);
    }
    let source_faces = source
        .material_faces
        .iter()
        .map(|face| face.face_id)
        .collect::<HashSet<_>>();
    let target_faces = geometry.face_ids().iter().copied().collect::<HashSet<_>>();
    let mapping = source_to_target.iter().copied().collect::<HashMap<_, _>>();
    if mapping.len() != source_to_target.len()
        || mapping.keys().copied().collect::<HashSet<_>>() != source_faces
        || mapping.values().copied().collect::<HashSet<_>>() != target_faces
    {
        return Err(ContinuousLayerTransportErrorV1::BindingMismatch);
    }
    let expected = source
        .face_pair_orders
        .iter()
        .map(|order| {
            Ok((
                *mapping
                    .get(&order.lower_face.face_id)
                    .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?,
                *mapping
                    .get(&order.upper_face.face_id)
                    .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?,
            ))
        })
        .collect::<Result<HashSet<_>, _>>()?;
    preflight_continuous_layer_transport_work_v1(expected_transitions, expected.len(), limits)?;
    let mut hashes = Vec::with_capacity(expected_transitions);
    for orders in transition_orders {
        if orders.iter().any(|(lower, upper)| lower == upper) {
            return Err(ContinuousLayerTransportErrorV1::Collision);
        }
        let actual = orders.iter().copied().collect::<HashSet<_>>();
        if actual.len() != orders.len() || actual.len() != expected.len() {
            return Err(ContinuousLayerTransportErrorV1::AmbiguousOrder);
        }
        if actual != expected {
            if actual
                .iter()
                .any(|(lower, upper)| expected.contains(&(*upper, *lower)))
            {
                return Err(ContinuousLayerTransportErrorV1::Crossing);
            }
            return Err(ContinuousLayerTransportErrorV1::AmbiguousOrder);
        }
        hashes.push(hash_orders(orders));
    }
    Ok(ContinuousLayerTransportCertificateV1 {
        issuer: geometry.clone(),
        source_instance: source as *const LayerOrderSnapshot as usize,
        source_hash: hash_source(source),
        schedule_hash: schedule.certificate_binding_fingerprint_v1(),
        closure_hash: closure.partition_binding_fingerprint_v1(),
        target_order_hash: *hashes
            .last()
            .ok_or(ContinuousLayerTransportErrorV1::AmbiguousOrder)?,
        transition_hashes: hashes,
        pair_order_count: expected.len(),
    })
}

/// Derives every transition order from issuer-owned face poses. The caller
/// supplies only the lineage and one finite world-space separation axis; no
/// target order witness is accepted as input.
pub fn derive_continuous_layer_transport_from_poses_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    source: &LayerOrderSnapshot,
    source_to_target: &[(FaceId, FaceId)],
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    separation_axis: [f64; 3],
    tolerance: f64,
    limits: ContinuousLayerTransportLimitsV1,
) -> Result<ContinuousLayerTransportCertificateV1, ContinuousLayerTransportErrorV1> {
    let norm_squared = separation_axis
        .iter()
        .map(|value| value * value)
        .sum::<f64>();
    if !norm_squared.is_finite()
        || norm_squared <= 0.0
        || !tolerance.is_finite()
        || tolerance < 0.0
        || !schedule.matches_binding(geometry, audit, closure.fixed_face())
    {
        return Err(ContinuousLayerTransportErrorV1::BindingMismatch);
    }
    let inverse_norm = norm_squared.sqrt().recip();
    let axis = separation_axis.map(|value| value * inverse_norm);
    let mapping = source_to_target.iter().copied().collect::<HashMap<_, _>>();
    if mapping.len() != source_to_target.len() {
        return Err(ContinuousLayerTransportErrorV1::BindingMismatch);
    }
    let mut parameters = closure
        .leaves()
        .iter()
        .map(|(depth, index, _)| *index as f64 / 2_f64.powi(*depth as i32))
        .collect::<Vec<_>>();
    parameters.push(1.0);
    preflight_continuous_layer_transport_work_v1(
        parameters.len(),
        source.face_pair_orders.len(),
        limits,
    )?;
    let mut transition_orders = Vec::with_capacity(parameters.len());
    let mut pose_order_hashes = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let angles = schedule
            .evaluate(parameter)
            .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?;
        let pose = geometry
            .solve_closed(audit, closure.fixed_face(), &angles, tolerance.max(1.0e-12))
            .map_err(|_| ContinuousLayerTransportErrorV1::BindingMismatch)?;
        let mut orders = Vec::with_capacity(source.face_pair_orders.len());
        let mut pose_hash = Sha256::new();
        pose_hash.update(b"ORIGAMI2_DERIVED_CONTINUOUS_LAYER_ORDER_V1");
        pose_hash.update(parameter.to_bits().to_be_bytes());
        for expected in &source.face_pair_orders {
            let lower = *mapping
                .get(&expected.lower_face.face_id)
                .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?;
            let upper = *mapping
                .get(&expected.upper_face.face_id)
                .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?;
            let lower_projection = face_centroid_projection(geometry, &pose, lower, axis)?;
            let upper_projection = face_centroid_projection(geometry, &pose, upper, axis)?;
            let separation = upper_projection - lower_projection;
            if separation.abs() <= tolerance {
                return Err(ContinuousLayerTransportErrorV1::AmbiguousOrder);
            }
            if separation < 0.0 {
                return Err(ContinuousLayerTransportErrorV1::Crossing);
            }
            pose_hash.update(lower.canonical_bytes());
            pose_hash.update(upper.canonical_bytes());
            pose_hash.update(lower_projection.to_bits().to_be_bytes());
            pose_hash.update(upper_projection.to_bits().to_be_bytes());
            orders.push((lower, upper));
        }
        transition_orders.push(orders);
        pose_order_hashes.push(pose_hash.finalize().into());
    }
    let mut certificate = prove_continuous_layer_transport_v1(
        geometry,
        source,
        source_to_target,
        schedule,
        closure,
        &transition_orders,
        limits,
    )?;
    certificate.target_order_hash = *pose_order_hashes
        .last()
        .ok_or(ContinuousLayerTransportErrorV1::AmbiguousOrder)?;
    certificate.transition_hashes = pose_order_hashes;
    Ok(certificate)
}

fn face_centroid_projection(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ori_kinematics::ClosedMaterialHingeGraphPose,
    face: FaceId,
    axis: [f64; 3],
) -> Result<f64, ContinuousLayerTransportErrorV1> {
    let boundary = geometry
        .face_boundary_vertices(face)
        .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?;
    if boundary.is_empty() {
        return Err(ContinuousLayerTransportErrorV1::BindingMismatch);
    }
    let transform = pose
        .face_transform(face)
        .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?;
    let mut sum = [0.0; 3];
    for vertex in boundary {
        let point = geometry
            .vertex_position(*vertex)
            .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)?;
        let world = transform
            .apply_point(point)
            .map_err(|_| ContinuousLayerTransportErrorV1::BindingMismatch)?;
        sum[0] += world.x();
        sum[1] += world.y();
        sum[2] += world.z();
    }
    let divisor = boundary.len() as f64;
    let projection = (sum[0] * axis[0] + sum[1] * axis[1] + sum[2] * axis[2]) / divisor;
    projection
        .is_finite()
        .then_some(projection)
        .ok_or(ContinuousLayerTransportErrorV1::BindingMismatch)
}

fn hash_orders(orders: &[(FaceId, FaceId)]) -> [u8; 32] {
    let mut canonical = orders.to_vec();
    canonical.sort_unstable_by_key(|(a, b)| (a.canonical_bytes(), b.canonical_bytes()));
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_CONTINUOUS_LAYER_ORDER_V1");
    for (lower, upper) in canonical {
        hash.update(lower.canonical_bytes());
        hash.update(upper.canonical_bytes());
    }
    hash.finalize().into()
}

fn hash_source(source: &LayerOrderSnapshot) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_SOURCE_LAYER_ORDER_V1");
    hash.update(b"facewise_layer_order_v1");
    if let Some(namespace) = source.provenance.source.identity_namespace {
        hash.update([1]);
        hash.update(namespace.canonical_bytes());
    } else {
        hash.update([0]);
    }
    hash.update(source.provenance.source.source_revision.to_be_bytes());
    if let Some(fingerprint) = source.provenance.source.source_fingerprint {
        hash.update([1]);
        hash.update(fingerprint.0);
    } else {
        hash.update([0]);
    }
    hash.update((source.material_faces.len() as u64).to_be_bytes());
    for face in &source.material_faces {
        hash.update(face.face_id.canonical_bytes());
        hash.update(face.face_key.0);
    }
    for order in &source.face_pair_orders {
        hash.update(order.lower_face.face_id.canonical_bytes());
        hash.update(order.upper_face.face_id.canonical_bytes());
    }
    hash.finalize().into()
}
