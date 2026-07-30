use ori_core::{
    BeginnerGenerationDocumentAuthorityStatusV1, beginner_generation_document_authority_status_v1,
};

use crate::{FormatError, ProjectDocument};

/// Only applied history that contains the authority edge for the current
/// positive claim can substitute for a final-document binding. Unrelated
/// history, Redo-only Apply entries, and truncated history must not bypass
/// document-only authority admission.
pub(crate) fn has_authoritative_beginner_generation_history_v1(
    history: Option<&ori_core::EditorHistoryV1>,
    document: &ProjectDocument,
) -> bool {
    history.is_some_and(|history| {
        history.authenticates_current_beginner_generation_provenance_v1(
            &document.beginner_design_profile,
        )
    })
}

/// Requires every positive generation claim in a newly written
/// document-only artifact to carry a current final-document binding.
pub(crate) fn require_current_beginner_generation_document_authority_v1(
    document: &ProjectDocument,
) -> Result<(), FormatError> {
    match beginner_generation_document_authority_status_v1(
        &document.crease_pattern,
        &document.paper,
        &document.beginner_design_profile,
    ) {
        BeginnerGenerationDocumentAuthorityStatusV1::NoProvenance
        | BeginnerGenerationDocumentAuthorityStatusV1::Current => Ok(()),
        BeginnerGenerationDocumentAuthorityStatusV1::LegacyUnbound
        | BeginnerGenerationDocumentAuthorityStatusV1::Mismatch => {
            Err(FormatError::InvalidBeginnerDesignProfile)
        }
    }
}

/// Rejects a present stale binding while allowing replayable legacy history to
/// carry its pre-binding provenance shape.
pub(crate) fn reject_mismatched_beginner_generation_document_authority_v1(
    document: &ProjectDocument,
) -> Result<(), FormatError> {
    if beginner_generation_document_authority_status_v1(
        &document.crease_pattern,
        &document.paper,
        &document.beginner_design_profile,
    ) == BeginnerGenerationDocumentAuthorityStatusV1::Mismatch
    {
        Err(FormatError::InvalidBeginnerDesignProfile)
    } else {
        Ok(())
    }
}

/// Admits positive generation evidence from an untrusted document-only
/// artifact.
///
/// Legacy evidence without a final-document binding remains readable, but is
/// explicitly downgraded to an unproven profile. A present, stale binding is a
/// malformed positive claim and fails closed.
pub(crate) fn admit_beginner_generation_document_authority_v1(
    document: &mut ProjectDocument,
) -> Result<(), FormatError> {
    match beginner_generation_document_authority_status_v1(
        &document.crease_pattern,
        &document.paper,
        &document.beginner_design_profile,
    ) {
        BeginnerGenerationDocumentAuthorityStatusV1::NoProvenance
        | BeginnerGenerationDocumentAuthorityStatusV1::Current => Ok(()),
        BeginnerGenerationDocumentAuthorityStatusV1::LegacyUnbound => {
            document.beginner_design_profile.generation_provenance = None;
            Ok(())
        }
        BeginnerGenerationDocumentAuthorityStatusV1::Mismatch => {
            Err(FormatError::InvalidBeginnerDesignProfile)
        }
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::BeginnerGenerationProvenanceV1;

    use super::*;

    fn document_with_legacy_provenance() -> ProjectDocument {
        let mut document =
            ProjectDocument::new("authority fixture", ori_domain::CreasePattern::empty());
        document.beginner_design_profile.generation_provenance =
            Some(BeginnerGenerationProvenanceV1 {
                schema_version: 1,
                topology_authority_sha256: [0x11; 32],
                fold_path_certificate_sha256: Some([0x22; 32]),
                document_authority_sha256: None,
                confidence_score: 90,
                confidence_reasons: vec!["bounded_native_fold_path_v2".to_owned()],
                explicit_override: false,
                source_asset_fingerprint: "none".to_owned(),
                semantic_landmark_provenance: None,
                generic_tree: None,
                reference_consensus: None,
                reference_consensus_summary: None,
            });
        document
    }

    #[test]
    fn legacy_positive_claim_is_readable_only_after_downgrade() {
        let mut document = document_with_legacy_provenance();
        assert!(matches!(
            require_current_beginner_generation_document_authority_v1(&document),
            Err(FormatError::InvalidBeginnerDesignProfile)
        ));
        admit_beginner_generation_document_authority_v1(&mut document).unwrap();
        assert!(
            document
                .beginner_design_profile
                .generation_provenance
                .is_none()
        );
        require_current_beginner_generation_document_authority_v1(&document).unwrap();
    }

    #[test]
    fn stale_bound_positive_claim_fails_closed() {
        let mut document = document_with_legacy_provenance();
        ori_core::bind_beginner_generation_document_authority_v1(
            &document.crease_pattern,
            &document.paper,
            &mut document.beginner_design_profile,
        )
        .unwrap();
        document.paper.thickness_mm += 0.01;
        assert!(matches!(
            admit_beginner_generation_document_authority_v1(&mut document),
            Err(FormatError::InvalidBeginnerDesignProfile)
        ));
        assert!(
            document
                .beginner_design_profile
                .generation_provenance
                .is_some(),
            "a stale bound claim must not be silently converted into trusted absence"
        );
    }
}
