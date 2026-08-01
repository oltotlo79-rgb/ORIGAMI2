use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{BeginnerDesignProfileV1, beginner_design_profile_authority_sha256_v1};

/// Domain separator for the first persisted beginner-generation document
/// authority.
pub const BEGINNER_GENERATION_DOCUMENT_AUTHORITY_SHA256_V1_DOMAIN: &[u8] =
    b"ORIGAMI2\0beginner-generation-document-authority\0v1\0";

#[derive(Serialize)]
struct BeginnerGenerationDocumentAuthorityRefV1<'a> {
    fold_model_fingerprint_sha256: [u8; 32],
    profile_authority_sha256: [u8; 32],
    provenance: &'a crate::BeginnerGenerationProvenanceV1,
}

/// Computes the stable authority that binds positive beginner-generation
/// evidence to a final fold model and its profile inputs.
///
/// `fold_model_fingerprint_sha256` must be the canonical V1 fold-model
/// fingerprint produced by the runtime geometry authority. The document
/// authority field itself is cleared from the serialized provenance so the
/// digest is non-recursive. All other provenance fields remain bound.
///
/// Returns `None` when the profile has no positive generation provenance.
#[must_use]
pub fn beginner_generation_document_authority_sha256_v1(
    fold_model_fingerprint_sha256: [u8; 32],
    profile: &BeginnerDesignProfileV1,
) -> Option<[u8; 32]> {
    let mut provenance = profile.generation_provenance.clone()?;
    provenance.document_authority_sha256 = None;
    let authority = BeginnerGenerationDocumentAuthorityRefV1 {
        fold_model_fingerprint_sha256,
        profile_authority_sha256: beginner_design_profile_authority_sha256_v1(profile),
        provenance: &provenance,
    };
    let mut hasher = Sha256::new();
    hasher.update(BEGINNER_GENERATION_DOCUMENT_AUTHORITY_SHA256_V1_DOMAIN);
    serde_json::to_writer(Sha256Writer(&mut hasher), &authority)
        .expect("serializing bounded beginner-generation authority cannot fail");
    Some(hasher.finalize().into())
}

struct Sha256Writer<'a>(&'a mut Sha256);

impl std::io::Write for Sha256Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BeginnerGenerationProvenanceV1;

    fn profile() -> BeginnerDesignProfileV1 {
        BeginnerDesignProfileV1 {
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
        }
    }

    #[test]
    fn document_authority_binds_fold_profile_and_positive_provenance() {
        let profile = profile();
        let baseline =
            beginner_generation_document_authority_sha256_v1([0x33; 32], &profile).unwrap();
        assert_eq!(
            beginner_generation_document_authority_sha256_v1([0x33; 32], &profile),
            Some(baseline)
        );

        let mut changed_fold = [0x33; 32];
        changed_fold[0] ^= 1;
        assert_ne!(
            beginner_generation_document_authority_sha256_v1(changed_fold, &profile),
            Some(baseline)
        );

        let mut changed_profile = profile.clone();
        changed_profile.shape_fidelity_weight -= 1;
        changed_profile.foldability_weight += 1;
        assert_ne!(
            beginner_generation_document_authority_sha256_v1([0x33; 32], &changed_profile,),
            Some(baseline)
        );

        let mut changed_provenance = profile.clone();
        changed_provenance
            .generation_provenance
            .as_mut()
            .unwrap()
            .topology_authority_sha256[0] ^= 1;
        assert_ne!(
            beginner_generation_document_authority_sha256_v1([0x33; 32], &changed_provenance,),
            Some(baseline)
        );
    }

    #[test]
    fn document_authority_excludes_only_its_recursive_storage_field() {
        let mut profile = profile();
        let expected =
            beginner_generation_document_authority_sha256_v1([0x44; 32], &profile).unwrap();
        profile
            .generation_provenance
            .as_mut()
            .unwrap()
            .document_authority_sha256 = Some([0xff; 32]);
        assert_eq!(
            beginner_generation_document_authority_sha256_v1([0x44; 32], &profile),
            Some(expected)
        );
        profile.generation_provenance = None;
        assert_eq!(
            beginner_generation_document_authority_sha256_v1([0x44; 32], &profile),
            None
        );
    }
}
