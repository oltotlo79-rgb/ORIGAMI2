use ori_domain::{
    BeginnerDesignProfileV1, BeginnerGenerationProvenanceV1, CreasePattern, EdgeLayerAssignmentV1,
    LayerContentKindV1, LayerRecordV1, LengthDisplayUnit, Paper, UnderlayDocumentV1, UnderlayId,
    beginner_design_profile_authority_sha256_v1, validate_underlay_document_v1,
};

use super::{Command, EditorState, Inverse};

/// Exact pre-command state needed to make invalidation of persisted beginner
/// generation evidence part of the same Undo/Redo history edge.
///
/// This is captured only while live provenance exists and only for commands
/// that can change an input used by that provenance. Canonical fingerprints
/// and bounded authority fields avoid retaining a second full document while
/// the command is applied.
pub(super) struct BeginnerGenerationAuthoritySnapshot {
    fold_authority_fingerprint: String,
    edge_assignments: Vec<EdgeLayerAssignmentV1>,
    profile_authority_sha256: [u8; 32],
    reference_image_underlay_id: Option<UnderlayId>,
    referenced_underlay_asset: Option<Option<ori_domain::AssetId>>,
    provenance: Box<BeginnerGenerationProvenanceV1>,
}

impl BeginnerGenerationAuthoritySnapshot {
    pub(super) fn capture(editor: &EditorState) -> Self {
        let reference_image_underlay_id =
            beginner_generation_reference_image_underlay_id(&editor.beginner_design_profile);
        Self {
            fold_authority_fingerprint: beginner_generation_fold_authority_fingerprint(
                &editor.pattern,
                &editor.paper,
            ),
            edge_assignments: editor.project_layers.edge_assignments.clone(),
            profile_authority_sha256: beginner_design_profile_authority_sha256_v1(
                &editor.beginner_design_profile,
            ),
            reference_image_underlay_id,
            referenced_underlay_asset: beginner_generation_referenced_underlay_asset(
                &editor.underlays,
                reference_image_underlay_id,
            ),
            provenance: Box::new(
                editor
                    .beginner_design_profile
                    .generation_provenance
                    .clone()
                    .expect("the authority snapshot is captured only with live provenance"),
            ),
        }
    }

    pub(super) fn authority_changed(&self, editor: &EditorState) -> bool {
        self.fold_authority_fingerprint
            != beginner_generation_fold_authority_fingerprint(&editor.pattern, &editor.paper)
            || self.edge_assignments != editor.project_layers.edge_assignments
            || self.profile_authority_sha256
                != beginner_design_profile_authority_sha256_v1(&editor.beginner_design_profile)
            || self.referenced_underlay_asset
                != beginner_generation_referenced_underlay_asset(
                    &editor.underlays,
                    self.reference_image_underlay_id,
                )
    }

    pub(super) fn into_restore_parts(self) -> ([u8; 32], Box<BeginnerGenerationProvenanceV1>) {
        (self.profile_authority_sha256, self.provenance)
    }
}

fn beginner_generation_fold_authority_fingerprint(
    pattern: &CreasePattern,
    paper: &Paper,
) -> String {
    let mut authority_paper = paper.clone();
    authority_paper.cutting_allowed = false;
    authority_paper.front = Default::default();
    authority_paper.back = Default::default();
    authority_paper.length_display_unit = LengthDisplayUnit::Millimeter;
    crate::fold_model_fingerprint::fold_model_fingerprint_v1(pattern, &authority_paper)
}

pub(super) fn beginner_generation_provenance_escalates(
    current: &BeginnerDesignProfileV1,
    incoming: &BeginnerDesignProfileV1,
) -> bool {
    incoming.generation_provenance.is_some()
        && incoming.generation_provenance.as_ref() != current.generation_provenance.as_ref()
}

fn beginner_generation_reference_image_underlay_id(
    profile: &BeginnerDesignProfileV1,
) -> Option<UnderlayId> {
    let Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage { underlay_id, .. }) =
        profile.generation_constraints.target_asset
    else {
        return None;
    };
    Some(underlay_id)
}

fn beginner_generation_referenced_underlay_asset(
    underlays: &UnderlayDocumentV1,
    underlay_id: Option<UnderlayId>,
) -> Option<Option<ori_domain::AssetId>> {
    let underlay_id = underlay_id?;
    Some(
        underlays
            .underlays
            .iter()
            .find(|record| record.id == underlay_id)
            .map(|record| record.asset),
    )
}

pub(super) fn inverse_changes_beginner_design_profile(inverse: &Inverse) -> bool {
    match inverse {
        Inverse::RestoreStackedFoldDocument { .. }
        | Inverse::RestoreBeginnerDesignProfile { .. }
        | Inverse::RestoreUnderlaysAndBeginnerDesignProfile { .. }
        | Inverse::RestoreBeginnerGenerationProvenance { .. } => true,
        Inverse::Command(command) => command.changes_beginner_design_profile(),
        Inverse::RestoreMirrorSelection { .. }
        | Inverse::RestoreProjectMemo { .. }
        | Inverse::RestoreElementMetadata { .. }
        | Inverse::RestoreVertex { .. }
        | Inverse::RestoreEdge { .. }
        | Inverse::RestorePaperProperties { .. }
        | Inverse::RestoreLengthDisplayUnit { .. }
        | Inverse::RestoreVertexPositions { .. }
        | Inverse::RestoreBoundarySplit { .. }
        | Inverse::RestoreEdgeSplit { .. }
        | Inverse::RestoreEdgeIntersection { .. }
        | Inverse::RestoreTJunction { .. }
        | Inverse::RestoreIntersectionCluster { .. }
        | Inverse::RestoreBoundaryVertexRemoval { .. }
        | Inverse::RemoveAddedGeometricConstraint { .. }
        | Inverse::RestoreRemovedGeometricConstraint { .. }
        | Inverse::RemoveAddedInstructionStep { .. }
        | Inverse::RemoveAppendedInstructionSteps { .. }
        | Inverse::RestoreInstructionStepMetadata { .. }
        | Inverse::RestoreInstructionStepPose { .. }
        | Inverse::RestoreRemovedInstructionStep { .. }
        | Inverse::RestoreInstructionStepOrder { .. }
        | Inverse::RestoreDeletedLayer { .. } => false,
    }
}

pub(super) fn redo_beginner_profile_pre_state_matches(
    editor: &EditorState,
    inverse: &Inverse,
) -> bool {
    match inverse {
        Inverse::RestoreBeginnerGenerationProvenance {
            profile_authority_sha256,
            provenance,
            ..
        } => {
            beginner_design_profile_authority_sha256_v1(&editor.beginner_design_profile)
                == *profile_authority_sha256
                && editor
                    .beginner_design_profile
                    .generation_provenance
                    .as_ref()
                    == Some(provenance.as_ref())
        }
        Inverse::RestoreStackedFoldDocument {
            beginner_design_profile,
            ..
        } => editor.beginner_design_profile == **beginner_design_profile,
        Inverse::RestoreBeginnerDesignProfile { profile } => {
            editor.beginner_design_profile == **profile
        }
        Inverse::RestoreUnderlaysAndBeginnerDesignProfile { underlays, profile } => {
            editor.underlays == *underlays && editor.beginner_design_profile == **profile
        }
        _ => true,
    }
}

pub(super) fn beginner_reference_binding_is_live(
    profile: &BeginnerDesignProfileV1,
    underlays: &UnderlayDocumentV1,
    layers: &[LayerRecordV1],
) -> bool {
    let Some(target) = profile.generation_constraints.target_asset else {
        return true;
    };
    let ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage {
        underlay_id,
        asset_id,
    } = target
    else {
        let ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id } = target else {
            unreachable!("the target reference enum is exhaustive")
        };
        return asset_id.canonical_bytes() != [0; 16];
    };
    if underlay_id.canonical_bytes() == [0; 16] || asset_id.canonical_bytes() == [0; 16] {
        return false;
    }
    let mut bound = underlays
        .underlays
        .iter()
        .filter(|record| record.id == underlay_id);
    let Some(record) = bound.next() else {
        return false;
    };
    bound.next().is_none()
        && record.asset == asset_id
        && layers.iter().any(|layer| {
            layer.id == record.layer && layer.content_kind == LayerContentKindV1::Underlay
        })
}

pub(super) fn beginner_reference_binding_is_live_after_inverse(
    editor: &EditorState,
    inverse: &Inverse,
) -> bool {
    let mut underlays = editor.underlays.clone();
    let mut layers = editor.project_layers.layers.clone();
    match inverse {
        Inverse::RestoreMirrorSelection { project_layers, .. } => {
            layers.clone_from(&project_layers.layers);
        }
        Inverse::Command(Command::AddUnderlay { record }) => {
            if underlays
                .underlays
                .iter()
                .any(|candidate| candidate.id == record.id)
            {
                return false;
            }
            underlays.underlays.push(record.clone());
        }
        Inverse::Command(Command::UpdateUnderlay { record }) => {
            let mut matches = underlays
                .underlays
                .iter_mut()
                .filter(|candidate| candidate.id == record.id);
            let Some(candidate) = matches.next() else {
                return false;
            };
            if matches.next().is_some() {
                return false;
            }
            candidate.clone_from(record);
        }
        Inverse::Command(Command::RemoveUnderlay { id }) => {
            let before = underlays.underlays.len();
            underlays.underlays.retain(|candidate| candidate.id != *id);
            if before != underlays.underlays.len().saturating_add(1) {
                return false;
            }
        }
        Inverse::RestoreDeletedLayer { index, layer, .. } => {
            if *index > layers.len() || layers.iter().any(|candidate| candidate.id == layer.id) {
                return false;
            }
            layers.insert(*index, layer.clone());
        }
        Inverse::Command(Command::DeleteLayer { layer }) => {
            let before = layers.len();
            layers.retain(|candidate| candidate.id != *layer);
            if before != layers.len().saturating_add(1) {
                return false;
            }
        }
        _ => {}
    }
    if validate_underlay_document_v1(&underlays).is_err() {
        return false;
    }
    beginner_reference_binding_is_live(&editor.beginner_design_profile, &underlays, &layers)
}
