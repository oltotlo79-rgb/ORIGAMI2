//! Cell transport bound to an admitted positive-thickness block certificate.

use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{
    CanonicalCycleScheduleV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};

use super::{
    GENERAL_CELL_TRANSPORT_TOLERANCE_V1, GeneralCellTransportErrorV1, GeneralCellTransportInputV1,
    GeneralCellTransportLimitsV1, GeneralMultiFaceCellTransportProofV1,
    certify_general_multi_face_cell_transport_with_peak_limit_v1,
};
use crate::continuous_path::common_articulation_positive_thickness_v2::AdmittedPositiveThicknessRetainedEvidenceV2;
use crate::{
    AdmittedPositiveThicknessContinuousCertificateV2,
    AdmittedPositiveThicknessContinuousRevalidationInputV2,
};

pub const ADMITTED_GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V2: &str =
    "admitted_general_multi_face_cell_transport_v2";

pub struct AdmittedGeneralCellTransportInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub source: &'a LayerOrderSnapshot,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub positive_continuous: &'a AdmittedPositiveThicknessContinuousCertificateV2,
    pub paper_thickness_mm: f64,
    pub limits: GeneralCellTransportLimitsV1,
}

/// Opaque block transport proof that cannot be downgraded into the legacy
/// multi-block input surface.
#[derive(Debug)]
pub struct AdmittedGeneralMultiFaceCellTransportProofV2 {
    inner: GeneralMultiFaceCellTransportProofV1,
    positive_binding: [u8; 32],
    parent_graph_admission_binding: [u8; 32],
    binding_fingerprint: [u8; 32],
}

pub(crate) struct AdmittedGeneralCellTransportRetainedEvidenceV2 {
    positive_binding: [u8; 32],
    parent_graph_admission_binding: [u8; 32],
    binding_fingerprint: [u8; 32],
}

impl AdmittedGeneralMultiFaceCellTransportProofV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        ADMITTED_GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V2
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub const fn positive_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.positive_binding
    }

    #[must_use]
    pub const fn parent_graph_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_graph_admission_binding
    }

    #[must_use]
    pub fn target_order_hash_v2(&self) -> [u8; 32] {
        self.inner.target_order_hash()
    }

    #[must_use]
    pub const fn pair_order_count_v2(&self) -> usize {
        self.inner.pair_order_count()
    }

    #[must_use]
    pub fn checked_deep_retained_bytes_v2(&self) -> Option<usize> {
        self.inner.checked_deep_retained_bytes_v1()?.checked_add(
            std::mem::size_of::<Self>()
                .checked_sub(std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>())?,
        )
    }

    #[must_use]
    pub fn is_for_v2(&self, input: AdmittedGeneralCellTransportInputV2<'_>) -> bool {
        let positive = input.positive_continuous;
        let graph_limits = positive.graph_limits_v2();
        let admission = positive.retained_parent_graph_admission_v2();
        self.positive_binding == positive.binding_fingerprint_v2()
            && self.parent_graph_admission_binding == admission.binding_fingerprint_v2()
            && positive.is_for_v2(AdmittedPositiveThicknessContinuousRevalidationInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                fixed_face: input.closure.fixed_face(),
                schedule: input.schedule,
                closure: input.closure,
                paper_thickness_mm: input.paper_thickness_mm,
                graph_limits,
                admission,
            })
            && self.inner.is_for(
                input.geometry,
                input.source,
                input.schedule,
                input.closure,
                input.paper_thickness_mm,
            )
            && self.binding_fingerprint
                == admitted_transport_binding_v2(
                    &self.inner,
                    self.positive_binding,
                    self.parent_graph_admission_binding,
                )
    }

    pub(crate) fn into_parts_for_complete_v2(
        self,
    ) -> (
        GeneralMultiFaceCellTransportProofV1,
        AdmittedGeneralCellTransportRetainedEvidenceV2,
    ) {
        (
            self.inner,
            AdmittedGeneralCellTransportRetainedEvidenceV2 {
                positive_binding: self.positive_binding,
                parent_graph_admission_binding: self.parent_graph_admission_binding,
                binding_fingerprint: self.binding_fingerprint,
            },
        )
    }
}

impl AdmittedGeneralCellTransportRetainedEvidenceV2 {
    pub(crate) const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    pub(crate) const fn parent_graph_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_graph_admission_binding
    }

    pub(crate) fn is_for_v2(
        &self,
        inner: &GeneralMultiFaceCellTransportProofV1,
        positive: &AdmittedPositiveThicknessRetainedEvidenceV2,
    ) -> bool {
        self.positive_binding == positive.binding_fingerprint_v2()
            && self.parent_graph_admission_binding
                == positive.parent_graph_admission_binding_fingerprint_v2()
            && self.binding_fingerprint
                == admitted_transport_binding_v2(
                    inner,
                    self.positive_binding,
                    self.parent_graph_admission_binding,
                )
    }

    pub(crate) const fn checked_deep_retained_bytes_v2(&self) -> Option<usize> {
        Some(std::mem::size_of::<Self>())
    }
}

pub fn certify_admitted_general_multi_face_cell_transport_v2(
    input: AdmittedGeneralCellTransportInputV2<'_>,
) -> Result<AdmittedGeneralMultiFaceCellTransportProofV2, GeneralCellTransportErrorV1> {
    let positive = input.positive_continuous;
    let graph_limits = positive.graph_limits_v2();
    let admission = positive.retained_parent_graph_admission_v2();
    if !positive.is_for_v2(AdmittedPositiveThicknessContinuousRevalidationInputV2 {
        geometry: input.geometry,
        audit: input.audit,
        fixed_face: input.closure.fixed_face(),
        schedule: input.schedule,
        closure: input.closure,
        paper_thickness_mm: input.paper_thickness_mm,
        graph_limits,
        admission,
    }) {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }
    let outer_delta = std::mem::size_of::<AdmittedGeneralMultiFaceCellTransportProofV2>()
        .checked_sub(std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>())
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    let maximum_inner_peak = ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES
        .checked_sub(outer_delta)
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    let inner = certify_general_multi_face_cell_transport_with_peak_limit_v1(
        GeneralCellTransportInputV1 {
            geometry: input.geometry,
            audit: input.audit,
            source: input.source,
            schedule: input.schedule,
            closure: input.closure,
            positive_continuous: positive.inner_v2(),
            paper_thickness_mm: input.paper_thickness_mm,
            tolerance: GENERAL_CELL_TRANSPORT_TOLERANCE_V1,
            limits: input.limits,
        },
        maximum_inner_peak,
    )?;
    let positive_binding = positive.binding_fingerprint_v2();
    let parent_graph_admission_binding = admission.binding_fingerprint_v2();
    let binding_fingerprint =
        admitted_transport_binding_v2(&inner, positive_binding, parent_graph_admission_binding);
    Ok(AdmittedGeneralMultiFaceCellTransportProofV2 {
        inner,
        positive_binding,
        parent_graph_admission_binding,
        binding_fingerprint,
    })
}

fn admitted_transport_binding_v2(
    inner: &GeneralMultiFaceCellTransportProofV1,
    positive_binding: [u8; 32],
    parent_graph_admission_binding: [u8; 32],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(ADMITTED_GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V2.as_bytes());
    hash.update(inner.model_id().as_bytes());
    hash.update(inner.paper_thickness_mm().to_bits().to_le_bytes());
    hash.update((inner.pair_order_count() as u64).to_le_bytes());
    hash.update((inner.transition_hashes().len() as u64).to_le_bytes());
    for transition in inner.transition_hashes() {
        hash.update(transition);
    }
    hash.update(inner.target_order_hash());
    hash.update(positive_binding);
    hash.update(parent_graph_admission_binding);
    hash.finalize().into()
}
