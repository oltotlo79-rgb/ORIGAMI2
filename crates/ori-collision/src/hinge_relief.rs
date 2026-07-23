use std::collections::HashSet;

use ori_domain::EdgeId;
use ori_kinematics::MaterialHingeGraphGeometry;
use thiserror::Error;

pub const HINGE_RELIEF_POLICY_MODEL_ID_V1: &str = "explicit_hinge_relief_prerequisite_v1";
pub const MAX_HINGE_RELIEF_RECORDS_V1: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HingeReliefPolicyRecordV1 {
    pub edge: EdgeId,
    pub cutout_width_mm: f64,
    pub bevel_angle_degrees: f64,
    pub material_thickness_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HingeReliefPolicyLimitsV1 {
    pub max_records: usize,
}

impl Default for HingeReliefPolicyLimitsV1 {
    fn default() -> Self {
        Self {
            max_records: MAX_HINGE_RELIEF_RECORDS_V1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum HingeReliefPolicyErrorV1 {
    #[error("hinge relief record limit is invalid")]
    InvalidLimit,
    #[error("hinge relief record limit exceeded")]
    ResourceLimit,
    #[error("hinge relief records are not in canonical edge order")]
    NonCanonicalOrder,
    #[error("hinge relief edge is duplicated")]
    DuplicateEdge,
    #[error("hinge relief edge is not present in the bound material graph")]
    UnknownHinge,
    #[error("hinge relief dimensions must be finite and positive")]
    InvalidDimension,
    #[error("hinge relief bevel angle must be finite and strictly between 0 and 180 degrees")]
    InvalidBevelAngle,
    #[error("hinge relief thickness does not exactly match the material thickness")]
    ThicknessMismatch,
    #[error("hinge relief cutout is too narrow for its thickness and bevel angle")]
    InsufficientCutout,
    #[error("hinge relief prerequisite is not bound to this graph or policy")]
    BindingMismatch,
}

/// Opaque, observation-only prerequisite for a future shared-hinge corridor
/// proof. It never authorizes a project mutation or admits a collision pair.
#[derive(Debug, Clone)]
pub struct NativeHingeReliefPrerequisiteV1 {
    graph: MaterialHingeGraphGeometry,
    material_thickness_bits: u64,
    records: Vec<HingeReliefPolicyRecordV1>,
}

impl NativeHingeReliefPrerequisiteV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        HINGE_RELIEF_POLICY_MODEL_ID_V1
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_shared_hinge_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub fn records(&self) -> &[HingeReliefPolicyRecordV1] {
        &self.records
    }
}

pub fn prepare_hinge_relief_prerequisite_v1(
    graph: &MaterialHingeGraphGeometry,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<NativeHingeReliefPrerequisiteV1, HingeReliefPolicyErrorV1> {
    let hinge_edges = graph.hinges().iter().map(|hinge| hinge.edge());
    validate_policy(hinge_edges, material_thickness_mm, records, limits)?;
    Ok(NativeHingeReliefPrerequisiteV1 {
        graph: graph.clone(),
        material_thickness_bits: material_thickness_mm.to_bits(),
        records: records.to_vec(),
    })
}

pub fn revalidate_hinge_relief_prerequisite_v1(
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    graph: &MaterialHingeGraphGeometry,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<(), HingeReliefPolicyErrorV1> {
    validate_policy(
        graph.hinges().iter().map(|hinge| hinge.edge()),
        material_thickness_mm,
        records,
        limits,
    )?;
    if !prerequisite.graph.same_instance(graph)
        || prerequisite.material_thickness_bits != material_thickness_mm.to_bits()
        || prerequisite.records != records
    {
        return Err(HingeReliefPolicyErrorV1::BindingMismatch);
    }
    Ok(())
}

fn validate_policy(
    hinge_edges: impl IntoIterator<Item = EdgeId>,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<(), HingeReliefPolicyErrorV1> {
    if limits.max_records > MAX_HINGE_RELIEF_RECORDS_V1 {
        return Err(HingeReliefPolicyErrorV1::InvalidLimit);
    }
    if records.len() > limits.max_records {
        return Err(HingeReliefPolicyErrorV1::ResourceLimit);
    }
    if !material_thickness_mm.is_finite() || material_thickness_mm <= 0.0 {
        return Err(HingeReliefPolicyErrorV1::InvalidDimension);
    }
    let hinges = hinge_edges.into_iter().collect::<HashSet<_>>();
    for pair in records.windows(2) {
        let order = pair[0]
            .edge
            .canonical_bytes()
            .cmp(&pair[1].edge.canonical_bytes());
        if order.is_gt() {
            return Err(HingeReliefPolicyErrorV1::NonCanonicalOrder);
        }
        if order.is_eq() {
            return Err(HingeReliefPolicyErrorV1::DuplicateEdge);
        }
    }
    for record in records {
        if !hinges.contains(&record.edge) {
            return Err(HingeReliefPolicyErrorV1::UnknownHinge);
        }
        if !record.cutout_width_mm.is_finite()
            || record.cutout_width_mm <= 0.0
            || !record.material_thickness_mm.is_finite()
            || record.material_thickness_mm <= 0.0
        {
            return Err(HingeReliefPolicyErrorV1::InvalidDimension);
        }
        if record.material_thickness_mm.to_bits() != material_thickness_mm.to_bits() {
            return Err(HingeReliefPolicyErrorV1::ThicknessMismatch);
        }
        if !record.bevel_angle_degrees.is_finite()
            || !(0.0..180.0).contains(&record.bevel_angle_degrees)
            || record.bevel_angle_degrees == 0.0
        {
            return Err(HingeReliefPolicyErrorV1::InvalidBevelAngle);
        }
        let half_angle = (record.bevel_angle_degrees * 0.5).to_radians();
        let required_width = record.material_thickness_mm / (2.0 * half_angle.tan());
        if !required_width.is_finite() || record.cutout_width_mm < required_width {
            return Err(HingeReliefPolicyErrorV1::InsufficientCutout);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted_edges(count: usize) -> Vec<EdgeId> {
        let mut edges = (0..count).map(|_| EdgeId::new()).collect::<Vec<_>>();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        edges
    }

    fn records(edges: &[EdgeId]) -> Vec<HingeReliefPolicyRecordV1> {
        edges
            .iter()
            .map(|&edge| HingeReliefPolicyRecordV1 {
                edge,
                cutout_width_mm: 0.1,
                bevel_angle_degrees: 90.0,
                material_thickness_mm: 0.1,
            })
            .collect()
    }

    #[test]
    fn policy_validation_is_bounded_canonical_and_complete_at_four_eight_sixteen() {
        for count in [4, 8, 16] {
            let edges = sorted_edges(count);
            let records = records(&edges);
            validate_policy(
                edges.clone(),
                0.1,
                &records,
                HingeReliefPolicyLimitsV1::default(),
            )
            .unwrap();
        }
        let edges = sorted_edges(MAX_HINGE_RELIEF_RECORDS_V1 + 1);
        assert_eq!(
            validate_policy(
                edges.clone(),
                0.1,
                &records(&edges),
                HingeReliefPolicyLimitsV1::default(),
            ),
            Err(HingeReliefPolicyErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn policy_rejects_tamper_and_accepts_empty_default_compatibility() {
        validate_policy(
            std::iter::empty(),
            0.1,
            &[],
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();
        let edges = sorted_edges(2);
        let mut input = records(&edges);
        input.reverse();
        assert_eq!(
            validate_policy(
                edges.clone(),
                0.1,
                &input,
                HingeReliefPolicyLimitsV1::default()
            ),
            Err(HingeReliefPolicyErrorV1::NonCanonicalOrder)
        );
        let mut input = records(&edges);
        input[0].material_thickness_mm = 0.2;
        assert_eq!(
            validate_policy(edges, 0.1, &input, HingeReliefPolicyLimitsV1::default()),
            Err(HingeReliefPolicyErrorV1::ThicknessMismatch)
        );
    }
}
