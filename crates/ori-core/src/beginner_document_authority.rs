use ori_domain::{
    BeginnerDesignProfileV1, CreasePattern, Paper,
    beginner_generation_document_authority_sha256_v1, validate_beginner_design_profile_v1,
};

/// Result of comparing persisted beginner-generation evidence with the live
/// fold document and profile authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginnerGenerationDocumentAuthorityStatusV1 {
    /// The profile does not claim positive generation evidence.
    NoProvenance,
    /// Legacy positive evidence has no document binding and must be downgraded
    /// at an untrusted document-only persistence boundary.
    LegacyUnbound,
    /// The persisted binding exactly matches the live fold model, profile
    /// authority, and positive provenance fields.
    Current,
    /// A binding is present but does not match the live authority.
    Mismatch,
}

/// Computes the expected document binding for the profile's positive
/// generation evidence.
///
/// This function does not grant authority. Only an already-authorized atomic
/// generation Apply may persist the returned value.
#[must_use]
pub fn expected_beginner_generation_document_authority_sha256_v1(
    pattern: &CreasePattern,
    paper: &Paper,
    profile: &BeginnerDesignProfileV1,
) -> Option<[u8; 32]> {
    let mut authority_paper = paper.clone();
    // Cutting permission is an editing policy, not an input to the generated
    // crease geometry. Runtime provenance invalidation applies the same
    // normalization.
    authority_paper.cutting_allowed = false;
    let fold_model_fingerprint =
        ori_foldability::fold_model_fingerprint_v1(pattern, &authority_paper).0;
    beginner_generation_document_authority_sha256_v1(fold_model_fingerprint, profile)
}

/// Compares positive provenance with the final persisted fold document.
#[must_use]
pub fn beginner_generation_document_authority_status_v1(
    pattern: &CreasePattern,
    paper: &Paper,
    profile: &BeginnerDesignProfileV1,
) -> BeginnerGenerationDocumentAuthorityStatusV1 {
    let Some(provenance) = profile.generation_provenance.as_ref() else {
        return BeginnerGenerationDocumentAuthorityStatusV1::NoProvenance;
    };
    let Some(actual) = provenance.document_authority_sha256 else {
        return BeginnerGenerationDocumentAuthorityStatusV1::LegacyUnbound;
    };
    if expected_beginner_generation_document_authority_sha256_v1(pattern, paper, profile)
        == Some(actual)
    {
        BeginnerGenerationDocumentAuthorityStatusV1::Current
    } else {
        BeginnerGenerationDocumentAuthorityStatusV1::Mismatch
    }
}

/// Binds an already-authorized positive provenance value to the final fold
/// document and profile inputs.
///
/// Returns `None` for a structurally invalid profile or one without positive
/// provenance. Callers must not use this helper to promote untrusted archived
/// provenance.
pub fn bind_beginner_generation_document_authority_v1(
    pattern: &CreasePattern,
    paper: &Paper,
    profile: &mut BeginnerDesignProfileV1,
) -> Option<[u8; 32]> {
    if !validate_beginner_design_profile_v1(profile) {
        return None;
    }
    let authority =
        expected_beginner_generation_document_authority_sha256_v1(pattern, paper, profile)?;
    profile
        .generation_provenance
        .as_mut()
        .expect("the expected authority exists only with provenance")
        .document_authority_sha256 = Some(authority);
    Some(authority)
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        BeginnerGenerationProvenanceV1, Edge, EdgeId, EdgeKind, LengthDisplayUnit, Point2, Vertex,
        VertexId,
    };

    use super::*;

    fn fixture() -> (CreasePattern, Paper, BeginnerDesignProfileV1) {
        let first = VertexId::new();
        let second = VertexId::new();
        let third = VertexId::new();
        let fourth = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(10.0, 0.0),
                },
                Vertex {
                    id: third,
                    position: Point2::new(0.0, 10.0),
                },
                Vertex {
                    id: fourth,
                    position: Point2::new(10.0, 10.0),
                },
            ],
            edges: vec![Edge {
                id: EdgeId::new(),
                start: first,
                end: second,
                kind: EdgeKind::Valley,
            }],
        };
        let paper = Paper {
            boundary_vertices: vec![first, second, fourth, third],
            thickness_mm: 0.1,
            ..Paper::default()
        };
        let profile = BeginnerDesignProfileV1 {
            generation_provenance: Some(BeginnerGenerationProvenanceV1 {
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
            }),
            ..Default::default()
        };
        (pattern, paper, profile)
    }

    #[test]
    fn binding_covers_geometry_physical_paper_profile_and_provenance() {
        let (pattern, paper, mut profile) = fixture();
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &paper, &profile),
            BeginnerGenerationDocumentAuthorityStatusV1::LegacyUnbound
        );
        let authority =
            bind_beginner_generation_document_authority_v1(&pattern, &paper, &mut profile).unwrap();
        assert_eq!(
            profile
                .generation_provenance
                .as_ref()
                .unwrap()
                .document_authority_sha256,
            Some(authority)
        );
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &paper, &profile),
            BeginnerGenerationDocumentAuthorityStatusV1::Current
        );

        let mut changed_pattern = pattern.clone();
        changed_pattern.vertices[0].position.x = -0.0;
        assert_eq!(
            beginner_generation_document_authority_status_v1(&changed_pattern, &paper, &profile,),
            BeginnerGenerationDocumentAuthorityStatusV1::Mismatch
        );

        let mut changed_paper = paper.clone();
        changed_paper.thickness_mm = 0.2;
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &changed_paper, &profile,),
            BeginnerGenerationDocumentAuthorityStatusV1::Mismatch
        );

        let mut changed_boundary = paper.clone();
        changed_boundary.boundary_vertices.swap(0, 1);
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &changed_boundary, &profile,),
            BeginnerGenerationDocumentAuthorityStatusV1::Mismatch
        );

        let mut changed_profile = profile.clone();
        changed_profile.shape_fidelity_weight -= 1;
        changed_profile.foldability_weight += 1;
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &paper, &changed_profile,),
            BeginnerGenerationDocumentAuthorityStatusV1::Mismatch
        );

        let mut changed_provenance = profile.clone();
        changed_provenance
            .generation_provenance
            .as_mut()
            .unwrap()
            .topology_authority_sha256[0] ^= 1;
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &paper, &changed_provenance,),
            BeginnerGenerationDocumentAuthorityStatusV1::Mismatch
        );
    }

    #[test]
    fn presentation_only_paper_changes_preserve_document_authority() {
        let (pattern, paper, mut profile) = fixture();
        bind_beginner_generation_document_authority_v1(&pattern, &paper, &mut profile).unwrap();
        let mut presentation = paper.clone();
        presentation.front.color.red ^= 1;
        presentation.back.color.alpha ^= 1;
        presentation.length_display_unit = LengthDisplayUnit::Centimeter;
        presentation.cutting_allowed = !presentation.cutting_allowed;
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &presentation, &profile,),
            BeginnerGenerationDocumentAuthorityStatusV1::Current
        );
    }

    #[test]
    fn absent_or_invalid_provenance_cannot_be_bound() {
        let (pattern, paper, mut profile) = fixture();
        profile.generation_provenance = None;
        assert_eq!(
            bind_beginner_generation_document_authority_v1(&pattern, &paper, &mut profile),
            None
        );
        assert_eq!(
            beginner_generation_document_authority_status_v1(&pattern, &paper, &profile),
            BeginnerGenerationDocumentAuthorityStatusV1::NoProvenance
        );

        let (_, _, mut invalid) = fixture();
        invalid.shape_fidelity_weight = 0;
        assert_eq!(
            bind_beginner_generation_document_authority_v1(&pattern, &paper, &mut invalid),
            None
        );
    }
}
