//! Versioned, untrusted persistence boundary for editor Undo/Redo history.
//!
//! The runtime [`Command`] and [`Inverse`] enums deliberately remain free to
//! evolve. This module owns the stable V1 wire vocabulary and converts every
//! variant exhaustively, so adding a runtime variant fails compilation until
//! its persistence semantics are decided explicitly.

use ori_domain::ProjectId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::*;

pub const EDITOR_HISTORY_SCHEMA_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EditorHistoryErrorV1 {
    #[error("the editor history schema version is unsupported")]
    UnsupportedSchemaVersion,
    #[error("the editor history project ID must not be nil")]
    NilProjectId,
    #[error("the editor history entry limit is outside the supported range")]
    EntryLimitOutOfRange,
    #[error("the editor history contains too many Undo entries")]
    TooManyUndoEntries,
    #[error("the editor history contains too many Redo entries")]
    TooManyRedoEntries,
    #[error("an editor history index cannot be represented safely")]
    IndexOutOfRange,
    #[error("editor history contains a non-finite number")]
    NonFiniteNumber,
    #[error("an editor history inverse cannot be applied safely")]
    InvalidInverse,
    #[error("an editor history command cannot be replayed")]
    InvalidCommand,
    #[error("editor history does not reproduce the current document exactly")]
    CurrentDocumentMismatch,
    #[error("editor history inverse data is not the canonical inverse of its command")]
    InverseMismatch,
    #[error("editor history could not be encoded canonically")]
    EncodingFailed,
}

/// Opaque V1 editor history persisted by the `.ori2` adapter.
///
/// Fields stay private so callers cannot bypass the semantic replay performed
/// by [`EditorState::with_document_parts_and_history_v1`]. Serde still exposes
/// the strict versioned wire to `ori-formats`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorHistoryV1 {
    schema_version: u32,
    project_id: ProjectId,
    history_entry_limit: u32,
    undo_stack: Vec<HistoryEntryV1>,
    redo_stack: Vec<HistoryEntryV1>,
}

impl EditorHistoryV1 {
    #[must_use]
    pub const fn project_id(&self) -> ProjectId {
        self.project_id
    }

    #[must_use]
    pub const fn history_entry_limit(&self) -> u32 {
        self.history_entry_limit
    }

    #[must_use]
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    #[must_use]
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    #[must_use]
    pub fn is_default_empty(&self) -> bool {
        self.history_entry_limit == MAX_EDITOR_HISTORY_ENTRIES as u32
            && self.undo_stack.is_empty()
            && self.redo_stack.is_empty()
    }

    fn validate_shape(&self) -> Result<usize, EditorHistoryErrorV1> {
        if self.schema_version != EDITOR_HISTORY_SCHEMA_VERSION_V1 {
            return Err(EditorHistoryErrorV1::UnsupportedSchemaVersion);
        }
        if self.project_id.canonical_bytes() == [0; 16] {
            return Err(EditorHistoryErrorV1::NilProjectId);
        }
        let limit = usize::try_from(self.history_entry_limit)
            .map_err(|_| EditorHistoryErrorV1::EntryLimitOutOfRange)?;
        if !(1..=MAX_EDITOR_HISTORY_ENTRIES).contains(&limit) {
            return Err(EditorHistoryErrorV1::EntryLimitOutOfRange);
        }
        if self.undo_stack.len() > limit {
            return Err(EditorHistoryErrorV1::TooManyUndoEntries);
        }
        if self.redo_stack.len() > limit {
            return Err(EditorHistoryErrorV1::TooManyRedoEntries);
        }
        Ok(limit)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryEntryV1 {
    forward: CommandV1,
    inverse: InverseV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum JunctionVertexIntentV1 {
    Create { id: VertexId },
    Reuse { id: VertexId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntersectionEdgeTargetV1 {
    edge: EdgeId,
    new_edge: Option<EdgeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VertexPositionUpdateV1 {
    vertex: VertexId,
    position: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CommandV1 {
    UpdateProjectMemo {
        memo: String,
    },
    UpdateBeginnerDesignProfile {
        profile: BeginnerDesignProfileV1,
    },
    SetElementMetadata {
        target: ElementMetadataTargetV1,
        metadata: Option<ElementMetadataV1>,
    },
    AddVertex {
        id: VertexId,
        position: Point2,
    },
    MoveVertex {
        id: VertexId,
        position: Point2,
    },
    MoveEdge {
        id: EdgeId,
        start_position: Point2,
        end_position: Point2,
    },
    MoveVertices {
        updates: Vec<VertexPositionUpdateV1>,
    },
    RemoveVertex {
        id: VertexId,
    },
    AddEdge {
        id: EdgeId,
        start: VertexId,
        end: VertexId,
        edge_kind: EdgeKind,
    },
    AddConnectedVertex {
        vertex_id: VertexId,
        position: Point2,
        edge_id: EdgeId,
        start: VertexId,
        edge_kind: EdgeKind,
    },
    RemoveConnectedVertex {
        vertex_id: VertexId,
        edge_id: EdgeId,
    },
    RemoveEdge {
        id: EdgeId,
    },
    SetCuttingAllowed {
        allowed: bool,
    },
    UpdatePaperProperties {
        thickness_mm: f64,
        front_color: RgbaColor,
        back_color: RgbaColor,
        #[serde(default)]
        front_texture_asset: Option<ori_domain::AssetId>,
        #[serde(default)]
        back_texture_asset: Option<ori_domain::AssetId>,
        cutting_allowed: bool,
    },
    SetLengthDisplayUnit {
        unit: LengthDisplayUnit,
    },
    ResizeRectangularPaper {
        width_mm: f64,
        height_mm: f64,
    },
    SplitEdge {
        edge: EdgeId,
        new_vertex: VertexId,
        new_edge: EdgeId,
        fraction: f64,
    },
    ConnectEdgeIntersection {
        first_edge: EdgeId,
        second_edge: EdgeId,
        new_vertex: VertexId,
        first_new_edge: EdgeId,
        second_new_edge: EdgeId,
    },
    ConnectTJunction {
        first_edge: EdgeId,
        second_edge: EdgeId,
        new_edge: EdgeId,
    },
    ConnectIntersectionCluster {
        junction: JunctionVertexIntentV1,
        targets: Vec<IntersectionEdgeTargetV1>,
    },
    SplitBoundaryEdge {
        edge: EdgeId,
        new_vertex: VertexId,
        new_edge: EdgeId,
        fraction: f64,
    },
    RemoveBoundaryVertex {
        vertex: VertexId,
    },
    AddGeometricConstraint {
        record: GeometricConstraintRecordV1,
    },
    RemoveGeometricConstraint {
        id: ConstraintId,
    },
    AddAnnotation {
        record: AnnotationRecordV1,
    },
    UpdateAnnotation {
        record: AnnotationRecordV1,
    },
    RemoveAnnotation {
        id: AnnotationId,
    },
    AddUnderlay {
        record: UnderlayRecordV1,
    },
    UpdateUnderlay {
        record: UnderlayRecordV1,
    },
    RemoveUnderlay {
        id: UnderlayId,
    },
    AddInstructionStep {
        step: InstructionStep,
    },
    AppendInstructionSteps {
        steps: Vec<InstructionStep>,
    },
    UpdateInstructionStepMetadata {
        step_id: InstructionStepId,
        title: String,
        description: String,
        caution: String,
        duration_ms: u32,
        #[serde(default)]
        visual: InstructionVisual,
    },
    ReplaceInstructionStepPose {
        step_id: InstructionStepId,
        pose: InstructionPose,
    },
    RemoveInstructionStep {
        step_id: InstructionStepId,
    },
    MoveInstructionStep {
        step_id: InstructionStepId,
        target_index: u32,
    },
    CreateLayer {
        layer: LayerRecordV1,
        target_index: u32,
    },
    RenameLayer {
        layer: LayerId,
        name: String,
    },
    UpdateLayerPresentation {
        layer: LayerId,
        visible: bool,
        locked: bool,
        opacity: f64,
    },
    MoveLayer {
        layer: LayerId,
        target_index: u32,
    },
    DeleteLayer {
        layer: LayerId,
    },
    AssignEdgeToLayer {
        edge: EdgeId,
        layer: LayerId,
    },
    MirrorSelection {
        vertices: Vec<VertexId>,
        edges: Vec<EdgeId>,
        axis: MirrorAxisV1,
        mode: MirrorSelectionModeV1,
        new_vertices: Vec<VertexId>,
        new_edges: Vec<EdgeId>,
    },
    ApplyStackedFoldDocument {
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        project_layers: ProjectLayerDocumentV1,
        beginner_design_profile: BeginnerDesignProfileV1,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexedVertexV1 {
    index: u32,
    vertex: Vertex,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexedEdgeV1 {
    index: u32,
    edge: Edge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndexedEdgeLayerAssignmentV1 {
    index: u32,
    assignment: EdgeLayerAssignmentV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VertexPositionV1 {
    vertex: VertexId,
    position: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum InverseV1 {
    RestoreMirrorSelection {
        pattern: CreasePattern,
        project_layers: ProjectLayerDocumentV1,
    },
    RestoreStackedFoldDocument {
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        project_layers: ProjectLayerDocumentV1,
        beginner_design_profile: BeginnerDesignProfileV1,
    },
    RestoreProjectMemo {
        memo: String,
    },
    RestoreBeginnerDesignProfile {
        profile: BeginnerDesignProfileV1,
    },
    RestoreElementMetadata {
        target: ElementMetadataTargetV1,
        metadata: Option<ElementMetadataV1>,
    },
    Command {
        command: CommandV1,
    },
    RestoreVertex {
        index: u32,
        vertex: Vertex,
    },
    RestoreEdge {
        index: u32,
        edge: Edge,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layer_assignment: Option<IndexedEdgeLayerAssignmentV1>,
    },
    RestorePaperProperties {
        thickness_mm: f64,
        front_color: RgbaColor,
        back_color: RgbaColor,
        #[serde(default)]
        front_texture_asset: Option<ori_domain::AssetId>,
        #[serde(default)]
        back_texture_asset: Option<ori_domain::AssetId>,
        cutting_allowed: bool,
    },
    RestoreLengthDisplayUnit {
        unit: LengthDisplayUnit,
    },
    RestoreVertexPositions {
        vertices: Vec<VertexPositionV1>,
    },
    RestoreBoundarySplit {
        boundary_vertices: Vec<VertexId>,
        original_edge: IndexedEdgeV1,
        new_vertex: IndexedVertexV1,
        new_edge: IndexedEdgeV1,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_edge_assignment: Option<EdgeLayerAssignmentV1>,
    },
    RestoreEdgeSplit {
        original_edge: IndexedEdgeV1,
        new_vertex: IndexedVertexV1,
        new_edge: IndexedEdgeV1,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_edge_assignment: Option<EdgeLayerAssignmentV1>,
    },
    RestoreEdgeIntersection {
        original_edges: [IndexedEdgeV1; 2],
        new_edges: [IndexedEdgeV1; 2],
        new_vertex: IndexedVertexV1,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        new_edge_assignments: Vec<EdgeLayerAssignmentV1>,
    },
    RestoreTJunction {
        original_edge: IndexedEdgeV1,
        new_edge: IndexedEdgeV1,
        boundary_vertices: Option<Vec<VertexId>>,
        changed_vertices: [VertexId; 4],
        changed_edges: [EdgeId; 3],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_edge_assignment: Option<EdgeLayerAssignmentV1>,
    },
    RestoreIntersectionCluster {
        original_boundary_vertices: Option<Vec<VertexId>>,
        original_edges: Vec<IndexedEdgeV1>,
        inserted_edges: Vec<IndexedEdgeV1>,
        created_vertex: Option<IndexedVertexV1>,
        junction_vertex: VertexId,
        changed_vertices: Vec<VertexId>,
        changed_edges: Vec<EdgeId>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        new_edge_assignments: Vec<EdgeLayerAssignmentV1>,
    },
    RestoreBoundaryVertexRemoval {
        boundary_index: u32,
        vertex: IndexedVertexV1,
        kept_edge: IndexedEdgeV1,
        removed_edge: IndexedEdgeV1,
        previous_vertex: VertexId,
        next_vertex: VertexId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        removed_edge_assignment: Option<IndexedEdgeLayerAssignmentV1>,
    },
    RemoveAddedGeometricConstraint {
        id: ConstraintId,
    },
    RestoreRemovedGeometricConstraint {
        index: u32,
        record: GeometricConstraintRecordV1,
    },
    RemoveAddedInstructionStep {
        step_id: InstructionStepId,
    },
    RemoveAppendedInstructionSteps {
        step_ids: Vec<InstructionStepId>,
    },
    RestoreInstructionStepMetadata {
        step_id: InstructionStepId,
        title: String,
        description: String,
        caution: String,
        duration_ms: u32,
        #[serde(default)]
        visual: InstructionVisual,
    },
    RestoreInstructionStepPose {
        step_id: InstructionStepId,
        pose: InstructionPose,
    },
    RestoreRemovedInstructionStep {
        index: u32,
        step: InstructionStep,
    },
    RestoreInstructionStepOrder {
        step_id: InstructionStepId,
        previous_index: u32,
    },
    RestoreDeletedLayer {
        index: u32,
        layer: LayerRecordV1,
        assignments: Vec<IndexedEdgeLayerAssignmentV1>,
    },
}

fn index_to_wire(index: usize) -> Result<u32, EditorHistoryErrorV1> {
    u32::try_from(index).map_err(|_| EditorHistoryErrorV1::IndexOutOfRange)
}

fn index_from_wire(index: u32) -> Result<usize, EditorHistoryErrorV1> {
    usize::try_from(index).map_err(|_| EditorHistoryErrorV1::IndexOutOfRange)
}

fn junction_to_wire(junction: JunctionVertexIntent) -> JunctionVertexIntentV1 {
    match junction {
        JunctionVertexIntent::Create { id } => JunctionVertexIntentV1::Create { id },
        JunctionVertexIntent::Reuse { id } => JunctionVertexIntentV1::Reuse { id },
    }
}

fn junction_from_wire(junction: JunctionVertexIntentV1) -> JunctionVertexIntent {
    match junction {
        JunctionVertexIntentV1::Create { id } => JunctionVertexIntent::Create { id },
        JunctionVertexIntentV1::Reuse { id } => JunctionVertexIntent::Reuse { id },
    }
}

fn target_to_wire(target: &IntersectionEdgeTarget) -> IntersectionEdgeTargetV1 {
    IntersectionEdgeTargetV1 {
        edge: target.edge,
        new_edge: target.new_edge,
    }
}

fn target_from_wire(target: IntersectionEdgeTargetV1) -> IntersectionEdgeTarget {
    IntersectionEdgeTarget {
        edge: target.edge,
        new_edge: target.new_edge,
    }
}

fn command_to_wire(command: &Command) -> Result<CommandV1, EditorHistoryErrorV1> {
    Ok(match command {
        Command::UpdateProjectMemo { memo } => CommandV1::UpdateProjectMemo { memo: memo.clone() },
        Command::UpdateBeginnerDesignProfile { profile } => {
            CommandV1::UpdateBeginnerDesignProfile {
                profile: profile.clone(),
            }
        }
        Command::SetElementMetadata { target, metadata } => CommandV1::SetElementMetadata {
            target: *target,
            metadata: metadata.clone(),
        },
        Command::AddVertex { id, position } => CommandV1::AddVertex {
            id: *id,
            position: *position,
        },
        Command::MoveVertex { id, position } => CommandV1::MoveVertex {
            id: *id,
            position: *position,
        },
        Command::MoveEdge {
            id,
            start_position,
            end_position,
        } => CommandV1::MoveEdge {
            id: *id,
            start_position: *start_position,
            end_position: *end_position,
        },
        Command::MoveVertices { updates } => CommandV1::MoveVertices {
            updates: updates
                .iter()
                .map(|update| VertexPositionUpdateV1 {
                    vertex: update.vertex,
                    position: update.position,
                })
                .collect(),
        },
        Command::RemoveVertex { id } => CommandV1::RemoveVertex { id: *id },
        Command::AddEdge {
            id,
            start,
            end,
            kind,
        } => CommandV1::AddEdge {
            id: *id,
            start: *start,
            end: *end,
            edge_kind: *kind,
        },
        Command::AddConnectedVertex {
            vertex_id,
            position,
            edge_id,
            start,
            kind,
        } => CommandV1::AddConnectedVertex {
            vertex_id: *vertex_id,
            position: *position,
            edge_id: *edge_id,
            start: *start,
            edge_kind: *kind,
        },
        Command::RemoveConnectedVertex { vertex_id, edge_id } => CommandV1::RemoveConnectedVertex {
            vertex_id: *vertex_id,
            edge_id: *edge_id,
        },
        Command::RemoveEdge { id } => CommandV1::RemoveEdge { id: *id },
        Command::SetCuttingAllowed { allowed } => {
            CommandV1::SetCuttingAllowed { allowed: *allowed }
        }
        Command::UpdatePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            front_texture_asset,
            back_texture_asset,
            cutting_allowed,
        } => CommandV1::UpdatePaperProperties {
            thickness_mm: *thickness_mm,
            front_color: *front_color,
            back_color: *back_color,
            front_texture_asset: *front_texture_asset,
            back_texture_asset: *back_texture_asset,
            cutting_allowed: *cutting_allowed,
        },
        Command::SetLengthDisplayUnit { unit } => CommandV1::SetLengthDisplayUnit { unit: *unit },
        Command::ResizeRectangularPaper {
            width_mm,
            height_mm,
        } => CommandV1::ResizeRectangularPaper {
            width_mm: *width_mm,
            height_mm: *height_mm,
        },
        Command::SplitEdge {
            edge,
            new_vertex,
            new_edge,
            fraction,
        } => CommandV1::SplitEdge {
            edge: *edge,
            new_vertex: *new_vertex,
            new_edge: *new_edge,
            fraction: *fraction,
        },
        Command::ConnectEdgeIntersection {
            first_edge,
            second_edge,
            new_vertex,
            first_new_edge,
            second_new_edge,
        } => CommandV1::ConnectEdgeIntersection {
            first_edge: *first_edge,
            second_edge: *second_edge,
            new_vertex: *new_vertex,
            first_new_edge: *first_new_edge,
            second_new_edge: *second_new_edge,
        },
        Command::ConnectTJunction {
            first_edge,
            second_edge,
            new_edge,
        } => CommandV1::ConnectTJunction {
            first_edge: *first_edge,
            second_edge: *second_edge,
            new_edge: *new_edge,
        },
        Command::ConnectIntersectionCluster { junction, targets } => {
            CommandV1::ConnectIntersectionCluster {
                junction: junction_to_wire(*junction),
                targets: targets.iter().map(target_to_wire).collect(),
            }
        }
        Command::SplitBoundaryEdge {
            edge,
            new_vertex,
            new_edge,
            fraction,
        } => CommandV1::SplitBoundaryEdge {
            edge: *edge,
            new_vertex: *new_vertex,
            new_edge: *new_edge,
            fraction: *fraction,
        },
        Command::RemoveBoundaryVertex { vertex } => {
            CommandV1::RemoveBoundaryVertex { vertex: *vertex }
        }
        Command::AddGeometricConstraint { record } => CommandV1::AddGeometricConstraint {
            record: record.clone(),
        },
        Command::RemoveGeometricConstraint { id } => {
            CommandV1::RemoveGeometricConstraint { id: *id }
        }
        Command::AddAnnotation { record } => CommandV1::AddAnnotation {
            record: record.clone(),
        },
        Command::UpdateAnnotation { record } => CommandV1::UpdateAnnotation {
            record: record.clone(),
        },
        Command::RemoveAnnotation { id } => CommandV1::RemoveAnnotation { id: *id },
        Command::AddUnderlay { record } => CommandV1::AddUnderlay {
            record: record.clone(),
        },
        Command::UpdateUnderlay { record } => CommandV1::UpdateUnderlay {
            record: record.clone(),
        },
        Command::RemoveUnderlay { id } => CommandV1::RemoveUnderlay { id: *id },
        Command::AddInstructionStep { step } => {
            CommandV1::AddInstructionStep { step: step.clone() }
        }
        Command::AppendInstructionSteps { steps } => CommandV1::AppendInstructionSteps {
            steps: steps.clone(),
        },
        Command::UpdateInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
            visual,
        } => CommandV1::UpdateInstructionStepMetadata {
            step_id: *step_id,
            title: title.clone(),
            description: description.clone(),
            caution: caution.clone(),
            duration_ms: *duration_ms,
            visual: visual.clone(),
        },
        Command::ReplaceInstructionStepPose { step_id, pose } => {
            CommandV1::ReplaceInstructionStepPose {
                step_id: *step_id,
                pose: pose.clone(),
            }
        }
        Command::RemoveInstructionStep { step_id } => {
            CommandV1::RemoveInstructionStep { step_id: *step_id }
        }
        Command::MoveInstructionStep {
            step_id,
            target_index,
        } => CommandV1::MoveInstructionStep {
            step_id: *step_id,
            target_index: index_to_wire(*target_index)?,
        },
        Command::CreateLayer {
            layer,
            target_index,
        } => CommandV1::CreateLayer {
            layer: layer.clone(),
            target_index: index_to_wire(*target_index)?,
        },
        Command::RenameLayer { layer, name } => CommandV1::RenameLayer {
            layer: *layer,
            name: name.clone(),
        },
        Command::UpdateLayerPresentation {
            layer,
            visible,
            locked,
            opacity,
        } => CommandV1::UpdateLayerPresentation {
            layer: *layer,
            visible: *visible,
            locked: *locked,
            opacity: *opacity,
        },
        Command::MoveLayer {
            layer,
            target_index,
        } => CommandV1::MoveLayer {
            layer: *layer,
            target_index: index_to_wire(*target_index)?,
        },
        Command::DeleteLayer { layer } => CommandV1::DeleteLayer { layer: *layer },
        Command::AssignEdgeToLayer { edge, layer } => CommandV1::AssignEdgeToLayer {
            edge: *edge,
            layer: *layer,
        },
        Command::MirrorSelection {
            vertices,
            edges,
            axis,
            mode,
            new_vertices,
            new_edges,
        } => CommandV1::MirrorSelection {
            vertices: vertices.clone(),
            edges: edges.clone(),
            axis: *axis,
            mode: *mode,
            new_vertices: new_vertices.clone(),
            new_edges: new_edges.clone(),
        },
        Command::ApplyStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile,
        } => CommandV1::ApplyStackedFoldDocument {
            pattern: pattern.clone(),
            paper: paper.clone(),
            instruction_timeline: instruction_timeline.clone(),
            project_layers: project_layers.clone(),
            beginner_design_profile: beginner_design_profile.clone(),
        },
    })
}

fn command_from_wire(command: CommandV1) -> Result<Command, EditorHistoryErrorV1> {
    Ok(match command {
        CommandV1::UpdateProjectMemo { memo } => Command::UpdateProjectMemo { memo },
        CommandV1::UpdateBeginnerDesignProfile { profile } => {
            Command::UpdateBeginnerDesignProfile { profile }
        }
        CommandV1::SetElementMetadata { target, metadata } => {
            Command::SetElementMetadata { target, metadata }
        }
        CommandV1::AddVertex { id, position } => Command::AddVertex { id, position },
        CommandV1::MoveVertex { id, position } => Command::MoveVertex { id, position },
        CommandV1::MoveEdge {
            id,
            start_position,
            end_position,
        } => Command::MoveEdge {
            id,
            start_position,
            end_position,
        },
        CommandV1::MoveVertices { updates } => Command::MoveVertices {
            updates: updates
                .into_iter()
                .map(|update| VertexPositionUpdate {
                    vertex: update.vertex,
                    position: update.position,
                })
                .collect(),
        },
        CommandV1::RemoveVertex { id } => Command::RemoveVertex { id },
        CommandV1::AddEdge {
            id,
            start,
            end,
            edge_kind,
        } => Command::AddEdge {
            id,
            start,
            end,
            kind: edge_kind,
        },
        CommandV1::AddConnectedVertex {
            vertex_id,
            position,
            edge_id,
            start,
            edge_kind,
        } => Command::AddConnectedVertex {
            vertex_id,
            position,
            edge_id,
            start,
            kind: edge_kind,
        },
        CommandV1::RemoveConnectedVertex { vertex_id, edge_id } => {
            Command::RemoveConnectedVertex { vertex_id, edge_id }
        }
        CommandV1::RemoveEdge { id } => Command::RemoveEdge { id },
        CommandV1::SetCuttingAllowed { allowed } => Command::SetCuttingAllowed { allowed },
        CommandV1::UpdatePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            front_texture_asset,
            back_texture_asset,
            cutting_allowed,
        } => Command::UpdatePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            front_texture_asset,
            back_texture_asset,
            cutting_allowed,
        },
        CommandV1::SetLengthDisplayUnit { unit } => Command::SetLengthDisplayUnit { unit },
        CommandV1::ResizeRectangularPaper {
            width_mm,
            height_mm,
        } => Command::ResizeRectangularPaper {
            width_mm,
            height_mm,
        },
        CommandV1::SplitEdge {
            edge,
            new_vertex,
            new_edge,
            fraction,
        } => Command::SplitEdge {
            edge,
            new_vertex,
            new_edge,
            fraction,
        },
        CommandV1::ConnectEdgeIntersection {
            first_edge,
            second_edge,
            new_vertex,
            first_new_edge,
            second_new_edge,
        } => Command::ConnectEdgeIntersection {
            first_edge,
            second_edge,
            new_vertex,
            first_new_edge,
            second_new_edge,
        },
        CommandV1::ConnectTJunction {
            first_edge,
            second_edge,
            new_edge,
        } => Command::ConnectTJunction {
            first_edge,
            second_edge,
            new_edge,
        },
        CommandV1::ConnectIntersectionCluster { junction, targets } => {
            Command::ConnectIntersectionCluster {
                junction: junction_from_wire(junction),
                targets: targets.into_iter().map(target_from_wire).collect(),
            }
        }
        CommandV1::SplitBoundaryEdge {
            edge,
            new_vertex,
            new_edge,
            fraction,
        } => Command::SplitBoundaryEdge {
            edge,
            new_vertex,
            new_edge,
            fraction,
        },
        CommandV1::RemoveBoundaryVertex { vertex } => Command::RemoveBoundaryVertex { vertex },
        CommandV1::AddGeometricConstraint { record } => Command::AddGeometricConstraint { record },
        CommandV1::RemoveGeometricConstraint { id } => Command::RemoveGeometricConstraint { id },
        CommandV1::AddAnnotation { record } => Command::AddAnnotation { record },
        CommandV1::UpdateAnnotation { record } => Command::UpdateAnnotation { record },
        CommandV1::RemoveAnnotation { id } => Command::RemoveAnnotation { id },
        CommandV1::AddUnderlay { record } => Command::AddUnderlay { record },
        CommandV1::UpdateUnderlay { record } => Command::UpdateUnderlay { record },
        CommandV1::RemoveUnderlay { id } => Command::RemoveUnderlay { id },
        CommandV1::AddInstructionStep { step } => Command::AddInstructionStep { step },
        CommandV1::AppendInstructionSteps { steps } => Command::AppendInstructionSteps { steps },
        CommandV1::UpdateInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
            visual,
        } => Command::UpdateInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
            visual,
        },
        CommandV1::ReplaceInstructionStepPose { step_id, pose } => {
            Command::ReplaceInstructionStepPose { step_id, pose }
        }
        CommandV1::RemoveInstructionStep { step_id } => Command::RemoveInstructionStep { step_id },
        CommandV1::MoveInstructionStep {
            step_id,
            target_index,
        } => Command::MoveInstructionStep {
            step_id,
            target_index: index_from_wire(target_index)?,
        },
        CommandV1::CreateLayer {
            layer,
            target_index,
        } => Command::CreateLayer {
            layer,
            target_index: index_from_wire(target_index)?,
        },
        CommandV1::RenameLayer { layer, name } => Command::RenameLayer { layer, name },
        CommandV1::UpdateLayerPresentation {
            layer,
            visible,
            locked,
            opacity,
        } => Command::UpdateLayerPresentation {
            layer,
            visible,
            locked,
            opacity,
        },
        CommandV1::MoveLayer {
            layer,
            target_index,
        } => Command::MoveLayer {
            layer,
            target_index: index_from_wire(target_index)?,
        },
        CommandV1::DeleteLayer { layer } => Command::DeleteLayer { layer },
        CommandV1::AssignEdgeToLayer { edge, layer } => Command::AssignEdgeToLayer { edge, layer },
        CommandV1::MirrorSelection {
            vertices,
            edges,
            axis,
            mode,
            new_vertices,
            new_edges,
        } => Command::MirrorSelection {
            vertices,
            edges,
            axis,
            mode,
            new_vertices,
            new_edges,
        },
        CommandV1::ApplyStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile,
        } => Command::ApplyStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile,
        },
    })
}

fn indexed_vertex_to_wire(
    index: usize,
    vertex: &Vertex,
) -> Result<IndexedVertexV1, EditorHistoryErrorV1> {
    Ok(IndexedVertexV1 {
        index: index_to_wire(index)?,
        vertex: vertex.clone(),
    })
}

fn indexed_edge_to_wire(index: usize, edge: &Edge) -> Result<IndexedEdgeV1, EditorHistoryErrorV1> {
    Ok(IndexedEdgeV1 {
        index: index_to_wire(index)?,
        edge: edge.clone(),
    })
}

fn indexed_layer_assignment_to_wire(
    index: usize,
    assignment: EdgeLayerAssignmentV1,
) -> Result<IndexedEdgeLayerAssignmentV1, EditorHistoryErrorV1> {
    Ok(IndexedEdgeLayerAssignmentV1 {
        index: index_to_wire(index)?,
        assignment,
    })
}

fn indexed_vertex_from_wire(
    value: IndexedVertexV1,
) -> Result<(usize, Vertex), EditorHistoryErrorV1> {
    Ok((index_from_wire(value.index)?, value.vertex))
}

fn indexed_edge_from_wire(value: IndexedEdgeV1) -> Result<(usize, Edge), EditorHistoryErrorV1> {
    Ok((index_from_wire(value.index)?, value.edge))
}

fn indexed_layer_assignment_from_wire(
    value: IndexedEdgeLayerAssignmentV1,
) -> Result<(usize, EdgeLayerAssignmentV1), EditorHistoryErrorV1> {
    Ok((index_from_wire(value.index)?, value.assignment))
}

fn inverse_to_wire(inverse: &Inverse) -> Result<InverseV1, EditorHistoryErrorV1> {
    Ok(match inverse {
        Inverse::RestoreMirrorSelection {
            pattern,
            project_layers,
        } => InverseV1::RestoreMirrorSelection {
            pattern: pattern.clone(),
            project_layers: project_layers.clone(),
        },
        Inverse::RestoreStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile,
        } => InverseV1::RestoreStackedFoldDocument {
            pattern: pattern.clone(),
            paper: paper.clone(),
            instruction_timeline: instruction_timeline.clone(),
            project_layers: project_layers.clone(),
            beginner_design_profile: beginner_design_profile.clone(),
        },
        Inverse::RestoreProjectMemo { memo } => {
            InverseV1::RestoreProjectMemo { memo: memo.clone() }
        }
        Inverse::RestoreBeginnerDesignProfile { profile } => {
            InverseV1::RestoreBeginnerDesignProfile {
                profile: profile.clone(),
            }
        }
        Inverse::RestoreElementMetadata { target, metadata } => InverseV1::RestoreElementMetadata {
            target: *target,
            metadata: metadata.clone(),
        },
        Inverse::Command(command) => InverseV1::Command {
            command: command_to_wire(command)?,
        },
        Inverse::RestoreVertex { index, vertex } => InverseV1::RestoreVertex {
            index: index_to_wire(*index)?,
            vertex: vertex.clone(),
        },
        Inverse::RestoreEdge {
            index,
            edge,
            layer_assignment,
        } => InverseV1::RestoreEdge {
            index: index_to_wire(*index)?,
            edge: edge.clone(),
            layer_assignment: layer_assignment
                .map(|(index, assignment)| indexed_layer_assignment_to_wire(index, assignment))
                .transpose()?,
        },
        Inverse::RestorePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            front_texture_asset,
            back_texture_asset,
            cutting_allowed,
        } => InverseV1::RestorePaperProperties {
            thickness_mm: *thickness_mm,
            front_color: *front_color,
            back_color: *back_color,
            front_texture_asset: *front_texture_asset,
            back_texture_asset: *back_texture_asset,
            cutting_allowed: *cutting_allowed,
        },
        Inverse::RestoreLengthDisplayUnit { unit } => {
            InverseV1::RestoreLengthDisplayUnit { unit: *unit }
        }
        Inverse::RestoreVertexPositions { vertices } => InverseV1::RestoreVertexPositions {
            vertices: vertices
                .iter()
                .map(|(vertex, position)| VertexPositionV1 {
                    vertex: *vertex,
                    position: *position,
                })
                .collect(),
        },
        Inverse::RestoreBoundarySplit {
            boundary_vertices,
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
            new_edge_assignment,
        } => InverseV1::RestoreBoundarySplit {
            boundary_vertices: boundary_vertices.clone(),
            original_edge: indexed_edge_to_wire(*original_edge_index, original_edge)?,
            new_vertex: indexed_vertex_to_wire(*new_vertex_index, new_vertex)?,
            new_edge: indexed_edge_to_wire(*new_edge_index, new_edge)?,
            new_edge_assignment: *new_edge_assignment,
        },
        Inverse::RestoreEdgeSplit {
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
            new_edge_assignment,
        } => InverseV1::RestoreEdgeSplit {
            original_edge: indexed_edge_to_wire(*original_edge_index, original_edge)?,
            new_vertex: indexed_vertex_to_wire(*new_vertex_index, new_vertex)?,
            new_edge: indexed_edge_to_wire(*new_edge_index, new_edge)?,
            new_edge_assignment: *new_edge_assignment,
        },
        Inverse::RestoreEdgeIntersection {
            original_edges,
            new_edges,
            new_vertex_index,
            new_vertex,
            new_edge_assignments,
        } => InverseV1::RestoreEdgeIntersection {
            original_edges: [
                indexed_edge_to_wire(original_edges[0].0, &original_edges[0].1)?,
                indexed_edge_to_wire(original_edges[1].0, &original_edges[1].1)?,
            ],
            new_edges: [
                indexed_edge_to_wire(new_edges[0].0, &new_edges[0].1)?,
                indexed_edge_to_wire(new_edges[1].0, &new_edges[1].1)?,
            ],
            new_vertex: indexed_vertex_to_wire(*new_vertex_index, new_vertex)?,
            new_edge_assignments: new_edge_assignments.clone(),
        },
        Inverse::RestoreTJunction {
            original_edge_index,
            original_edge,
            new_edge_index,
            new_edge,
            boundary_vertices,
            changed_vertices,
            changed_edges,
            new_edge_assignment,
        } => InverseV1::RestoreTJunction {
            original_edge: indexed_edge_to_wire(*original_edge_index, original_edge)?,
            new_edge: indexed_edge_to_wire(*new_edge_index, new_edge)?,
            boundary_vertices: boundary_vertices.clone(),
            changed_vertices: *changed_vertices,
            changed_edges: *changed_edges,
            new_edge_assignment: *new_edge_assignment,
        },
        Inverse::RestoreIntersectionCluster {
            original_boundary_vertices,
            original_edges,
            inserted_edges,
            created_vertex,
            junction_vertex,
            changed_vertices,
            changed_edges,
            new_edge_assignments,
        } => InverseV1::RestoreIntersectionCluster {
            original_boundary_vertices: original_boundary_vertices.clone(),
            original_edges: original_edges
                .iter()
                .map(|(index, edge)| indexed_edge_to_wire(*index, edge))
                .collect::<Result<Vec<_>, _>>()?,
            inserted_edges: inserted_edges
                .iter()
                .map(|(index, edge)| indexed_edge_to_wire(*index, edge))
                .collect::<Result<Vec<_>, _>>()?,
            created_vertex: created_vertex
                .as_ref()
                .map(|(index, vertex)| indexed_vertex_to_wire(*index, vertex))
                .transpose()?,
            junction_vertex: *junction_vertex,
            changed_vertices: changed_vertices.clone(),
            changed_edges: changed_edges.clone(),
            new_edge_assignments: new_edge_assignments.clone(),
        },
        Inverse::RestoreBoundaryVertexRemoval {
            boundary_index,
            vertex_index,
            vertex,
            kept_edge_index,
            kept_edge,
            removed_edge_index,
            removed_edge,
            previous_vertex,
            next_vertex,
            removed_edge_assignment,
        } => InverseV1::RestoreBoundaryVertexRemoval {
            boundary_index: index_to_wire(*boundary_index)?,
            vertex: indexed_vertex_to_wire(*vertex_index, vertex)?,
            kept_edge: indexed_edge_to_wire(*kept_edge_index, kept_edge)?,
            removed_edge: indexed_edge_to_wire(*removed_edge_index, removed_edge)?,
            previous_vertex: *previous_vertex,
            next_vertex: *next_vertex,
            removed_edge_assignment: removed_edge_assignment
                .map(|(index, assignment)| indexed_layer_assignment_to_wire(index, assignment))
                .transpose()?,
        },
        Inverse::RemoveAddedGeometricConstraint { id } => {
            InverseV1::RemoveAddedGeometricConstraint { id: *id }
        }
        Inverse::RestoreRemovedGeometricConstraint { index, record } => {
            InverseV1::RestoreRemovedGeometricConstraint {
                index: index_to_wire(*index)?,
                record: record.clone(),
            }
        }
        Inverse::RemoveAddedInstructionStep { step_id } => {
            InverseV1::RemoveAddedInstructionStep { step_id: *step_id }
        }
        Inverse::RemoveAppendedInstructionSteps { step_ids } => {
            InverseV1::RemoveAppendedInstructionSteps {
                step_ids: step_ids.clone(),
            }
        }
        Inverse::RestoreInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
            visual,
        } => InverseV1::RestoreInstructionStepMetadata {
            step_id: *step_id,
            title: title.clone(),
            description: description.clone(),
            caution: caution.clone(),
            duration_ms: *duration_ms,
            visual: visual.clone(),
        },
        Inverse::RestoreInstructionStepPose { step_id, pose } => {
            InverseV1::RestoreInstructionStepPose {
                step_id: *step_id,
                pose: pose.clone(),
            }
        }
        Inverse::RestoreRemovedInstructionStep { index, step } => {
            InverseV1::RestoreRemovedInstructionStep {
                index: index_to_wire(*index)?,
                step: step.clone(),
            }
        }
        Inverse::RestoreInstructionStepOrder {
            step_id,
            previous_index,
        } => InverseV1::RestoreInstructionStepOrder {
            step_id: *step_id,
            previous_index: index_to_wire(*previous_index)?,
        },
        Inverse::RestoreDeletedLayer {
            index,
            layer,
            assignments,
        } => InverseV1::RestoreDeletedLayer {
            index: index_to_wire(*index)?,
            layer: layer.clone(),
            assignments: assignments
                .iter()
                .map(|(index, assignment)| indexed_layer_assignment_to_wire(*index, *assignment))
                .collect::<Result<Vec<_>, _>>()?,
        },
    })
}

fn inverse_from_wire(inverse: InverseV1) -> Result<Inverse, EditorHistoryErrorV1> {
    Ok(match inverse {
        InverseV1::RestoreMirrorSelection {
            pattern,
            project_layers,
        } => Inverse::RestoreMirrorSelection {
            pattern,
            project_layers,
        },
        InverseV1::RestoreStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile,
        } => Inverse::RestoreStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile,
        },
        InverseV1::RestoreProjectMemo { memo } => Inverse::RestoreProjectMemo { memo },
        InverseV1::RestoreBeginnerDesignProfile { profile } => {
            Inverse::RestoreBeginnerDesignProfile { profile }
        }
        InverseV1::RestoreElementMetadata { target, metadata } => {
            Inverse::RestoreElementMetadata { target, metadata }
        }
        InverseV1::Command { command } => Inverse::Command(command_from_wire(command)?),
        InverseV1::RestoreVertex { index, vertex } => Inverse::RestoreVertex {
            index: index_from_wire(index)?,
            vertex,
        },
        InverseV1::RestoreEdge {
            index,
            edge,
            layer_assignment,
        } => Inverse::RestoreEdge {
            index: index_from_wire(index)?,
            edge,
            layer_assignment: layer_assignment
                .map(indexed_layer_assignment_from_wire)
                .transpose()?,
        },
        InverseV1::RestorePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            front_texture_asset,
            back_texture_asset,
            cutting_allowed,
        } => Inverse::RestorePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            front_texture_asset,
            back_texture_asset,
            cutting_allowed,
        },
        InverseV1::RestoreLengthDisplayUnit { unit } => Inverse::RestoreLengthDisplayUnit { unit },
        InverseV1::RestoreVertexPositions { vertices } => Inverse::RestoreVertexPositions {
            vertices: vertices
                .into_iter()
                .map(|value| (value.vertex, value.position))
                .collect(),
        },
        InverseV1::RestoreBoundarySplit {
            boundary_vertices,
            original_edge,
            new_vertex,
            new_edge,
            new_edge_assignment,
        } => {
            let (original_edge_index, original_edge) = indexed_edge_from_wire(original_edge)?;
            let (new_vertex_index, new_vertex) = indexed_vertex_from_wire(new_vertex)?;
            let (new_edge_index, new_edge) = indexed_edge_from_wire(new_edge)?;
            Inverse::RestoreBoundarySplit {
                boundary_vertices,
                original_edge_index,
                original_edge,
                new_vertex_index,
                new_vertex,
                new_edge_index,
                new_edge,
                new_edge_assignment,
            }
        }
        InverseV1::RestoreEdgeSplit {
            original_edge,
            new_vertex,
            new_edge,
            new_edge_assignment,
        } => {
            let (original_edge_index, original_edge) = indexed_edge_from_wire(original_edge)?;
            let (new_vertex_index, new_vertex) = indexed_vertex_from_wire(new_vertex)?;
            let (new_edge_index, new_edge) = indexed_edge_from_wire(new_edge)?;
            Inverse::RestoreEdgeSplit {
                original_edge_index,
                original_edge,
                new_vertex_index,
                new_vertex,
                new_edge_index,
                new_edge,
                new_edge_assignment,
            }
        }
        InverseV1::RestoreEdgeIntersection {
            original_edges,
            new_edges,
            new_vertex,
            new_edge_assignments,
        } => Inverse::RestoreEdgeIntersection {
            original_edges: [
                indexed_edge_from_wire(original_edges[0].clone())?,
                indexed_edge_from_wire(original_edges[1].clone())?,
            ],
            new_edges: [
                indexed_edge_from_wire(new_edges[0].clone())?,
                indexed_edge_from_wire(new_edges[1].clone())?,
            ],
            new_vertex_index: index_from_wire(new_vertex.index)?,
            new_vertex: new_vertex.vertex,
            new_edge_assignments,
        },
        InverseV1::RestoreTJunction {
            original_edge,
            new_edge,
            boundary_vertices,
            changed_vertices,
            changed_edges,
            new_edge_assignment,
        } => {
            let (original_edge_index, original_edge) = indexed_edge_from_wire(original_edge)?;
            let (new_edge_index, new_edge) = indexed_edge_from_wire(new_edge)?;
            Inverse::RestoreTJunction {
                original_edge_index,
                original_edge,
                new_edge_index,
                new_edge,
                boundary_vertices,
                changed_vertices,
                changed_edges,
                new_edge_assignment,
            }
        }
        InverseV1::RestoreIntersectionCluster {
            original_boundary_vertices,
            original_edges,
            inserted_edges,
            created_vertex,
            junction_vertex,
            changed_vertices,
            changed_edges,
            new_edge_assignments,
        } => Inverse::RestoreIntersectionCluster {
            original_boundary_vertices,
            original_edges: original_edges
                .into_iter()
                .map(indexed_edge_from_wire)
                .collect::<Result<Vec<_>, _>>()?,
            inserted_edges: inserted_edges
                .into_iter()
                .map(indexed_edge_from_wire)
                .collect::<Result<Vec<_>, _>>()?,
            created_vertex: created_vertex.map(indexed_vertex_from_wire).transpose()?,
            junction_vertex,
            changed_vertices,
            changed_edges,
            new_edge_assignments,
        },
        InverseV1::RestoreBoundaryVertexRemoval {
            boundary_index,
            vertex,
            kept_edge,
            removed_edge,
            previous_vertex,
            next_vertex,
            removed_edge_assignment,
        } => {
            let (vertex_index, vertex) = indexed_vertex_from_wire(vertex)?;
            let (kept_edge_index, kept_edge) = indexed_edge_from_wire(kept_edge)?;
            let (removed_edge_index, removed_edge) = indexed_edge_from_wire(removed_edge)?;
            Inverse::RestoreBoundaryVertexRemoval {
                boundary_index: index_from_wire(boundary_index)?,
                vertex_index,
                vertex,
                kept_edge_index,
                kept_edge,
                removed_edge_index,
                removed_edge,
                previous_vertex,
                next_vertex,
                removed_edge_assignment: removed_edge_assignment
                    .map(indexed_layer_assignment_from_wire)
                    .transpose()?,
            }
        }
        InverseV1::RemoveAddedGeometricConstraint { id } => {
            Inverse::RemoveAddedGeometricConstraint { id }
        }
        InverseV1::RestoreRemovedGeometricConstraint { index, record } => {
            Inverse::RestoreRemovedGeometricConstraint {
                index: index_from_wire(index)?,
                record,
            }
        }
        InverseV1::RemoveAddedInstructionStep { step_id } => {
            Inverse::RemoveAddedInstructionStep { step_id }
        }
        InverseV1::RemoveAppendedInstructionSteps { step_ids } => {
            Inverse::RemoveAppendedInstructionSteps { step_ids }
        }
        InverseV1::RestoreInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
            visual,
        } => Inverse::RestoreInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
            visual,
        },
        InverseV1::RestoreInstructionStepPose { step_id, pose } => {
            Inverse::RestoreInstructionStepPose { step_id, pose }
        }
        InverseV1::RestoreRemovedInstructionStep { index, step } => {
            Inverse::RestoreRemovedInstructionStep {
                index: index_from_wire(index)?,
                step,
            }
        }
        InverseV1::RestoreInstructionStepOrder {
            step_id,
            previous_index,
        } => Inverse::RestoreInstructionStepOrder {
            step_id,
            previous_index: index_from_wire(previous_index)?,
        },
        InverseV1::RestoreDeletedLayer {
            index,
            layer,
            assignments,
        } => Inverse::RestoreDeletedLayer {
            index: index_from_wire(index)?,
            layer,
            assignments: assignments
                .into_iter()
                .map(indexed_layer_assignment_from_wire)
                .collect::<Result<Vec<_>, _>>()?,
        },
    })
}

fn entry_to_wire(entry: &HistoryEntry) -> Result<HistoryEntryV1, EditorHistoryErrorV1> {
    validate_command_finite(&entry.forward)?;
    validate_inverse_finite(&entry.inverse)?;
    Ok(HistoryEntryV1 {
        forward: command_to_wire(&entry.forward)?,
        inverse: inverse_to_wire(&entry.inverse)?,
    })
}

fn entry_from_wire(entry: HistoryEntryV1) -> Result<(Command, Inverse), EditorHistoryErrorV1> {
    let forward = command_from_wire(entry.forward)?;
    let inverse = inverse_from_wire(entry.inverse)?;
    validate_command_finite(&forward)?;
    validate_inverse_finite(&inverse)?;
    Ok((forward, inverse))
}

fn finite_point(point: Point2) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn validate_instruction_pose_finite(pose: &InstructionPose) -> Result<(), EditorHistoryErrorV1> {
    if pose
        .hinge_angles
        .iter()
        .any(|hinge| !hinge.angle_degrees.is_finite())
    {
        Err(EditorHistoryErrorV1::NonFiniteNumber)
    } else {
        Ok(())
    }
}

fn validate_instruction_step_finite(step: &InstructionStep) -> Result<(), EditorHistoryErrorV1> {
    validate_instruction_pose_finite(&step.pose)
}

fn validate_constraint_finite(
    record: &GeometricConstraintRecordV1,
) -> Result<(), EditorHistoryErrorV1> {
    let finite = match &record.constraint {
        GeometricConstraintKindV1::FixedLength { length_mm, .. } => length_mm.is_finite(),
        GeometricConstraintKindV1::FixedAngle { angle_degrees, .. }
        | GeometricConstraintKindV1::RotationalSymmetry { angle_degrees, .. } => {
            angle_degrees.is_finite()
        }
        GeometricConstraintKindV1::LengthRatio { ratio, .. } => ratio.is_finite(),
        GeometricConstraintKindV1::Horizontal { .. }
        | GeometricConstraintKindV1::Vertical { .. }
        | GeometricConstraintKindV1::EqualLength { .. }
        | GeometricConstraintKindV1::Parallel { .. }
        | GeometricConstraintKindV1::PointOnLine { .. }
        | GeometricConstraintKindV1::MirrorSymmetry { .. }
        | GeometricConstraintKindV1::AngleBisector { .. } => true,
    };
    if finite {
        Ok(())
    } else {
        Err(EditorHistoryErrorV1::NonFiniteNumber)
    }
}

fn validate_stacked_fold_document(
    pattern: &CreasePattern,
    paper: &Paper,
    timeline: &InstructionTimeline,
    layers: &ProjectLayerDocumentV1,
    error: EditorHistoryErrorV1,
) -> Result<(), EditorHistoryErrorV1> {
    if paper.thickness_mm.to_bits() != 0.0_f64.to_bits()
        || pattern
            .vertices
            .iter()
            .any(|vertex| !finite_point(vertex.position))
        || !validate_crease_pattern(pattern).is_valid()
        || !validate_paper(paper, pattern).is_valid()
        || validate_instruction_timeline(timeline).is_err()
        || validate_project_layer_document_against_pattern_v1(layers, pattern).is_err()
    {
        return Err(error);
    }
    for step in &timeline.steps {
        validate_instruction_step_finite(step)?;
    }
    Ok(())
}

fn validate_command_finite(command: &Command) -> Result<(), EditorHistoryErrorV1> {
    match command {
        Command::MirrorSelection { axis, .. } => {
            if !finite_point(axis.start) || !finite_point(axis.end) {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Command::ApplyStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            ..
        } => validate_stacked_fold_document(
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            EditorHistoryErrorV1::InvalidCommand,
        )?,
        Command::UpdateProjectMemo { memo } => {
            if memo.chars().count() > 16_000
                || memo.chars().any(|character| {
                    character.is_control() && !matches!(character, '\n' | '\r' | '\t')
                })
            {
                return Err(EditorHistoryErrorV1::InvalidCommand);
            }
        }
        Command::UpdateBeginnerDesignProfile { profile } => {
            if !validate_beginner_design_profile_v1(profile) {
                return Err(EditorHistoryErrorV1::InvalidCommand);
            }
        }
        Command::SetElementMetadata { metadata, .. } => {
            if let Some(metadata) = metadata {
                ori_domain::validate_element_metadata_v1(metadata)
                    .map_err(|_| EditorHistoryErrorV1::InvalidCommand)?;
            }
        }
        Command::AddVertex { position, .. }
        | Command::MoveVertex { position, .. }
        | Command::AddConnectedVertex { position, .. } => {
            if !finite_point(*position) {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Command::MoveEdge {
            start_position,
            end_position,
            ..
        } => {
            if !finite_point(*start_position) || !finite_point(*end_position) {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Command::MoveVertices { updates } => {
            if updates.is_empty() || updates.len() > DEFAULT_MAX_CONSTRAINT_VERTICES {
                return Err(EditorHistoryErrorV1::InvalidCommand);
            }
            if updates.iter().any(|update| !finite_point(update.position)) {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
            let mut vertices = std::collections::HashSet::with_capacity(updates.len());
            if updates.iter().any(|update| !vertices.insert(update.vertex)) {
                return Err(EditorHistoryErrorV1::InvalidCommand);
            }
        }
        Command::UpdatePaperProperties { thickness_mm, .. } => {
            if !thickness_mm.is_finite() {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Command::ResizeRectangularPaper {
            width_mm,
            height_mm,
        } => {
            if !width_mm.is_finite() || !height_mm.is_finite() {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Command::SplitEdge { fraction, .. } | Command::SplitBoundaryEdge { fraction, .. } => {
            if !fraction.is_finite() {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Command::AddGeometricConstraint { record } => validate_constraint_finite(record)?,
        Command::AddInstructionStep { step } => validate_instruction_step_finite(step)?,
        Command::AppendInstructionSteps { steps } => {
            if steps.is_empty() {
                return Err(EditorHistoryErrorV1::InvalidCommand);
            }
            for step in steps {
                validate_instruction_step_finite(step)?;
            }
        }
        Command::ReplaceInstructionStepPose { pose, .. } => {
            validate_instruction_pose_finite(pose)?;
        }
        Command::UpdateLayerPresentation { opacity, .. } => {
            if !opacity.is_finite() {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
            if opacity.to_bits() == (-0.0_f64).to_bits()
                || !(ori_domain::MIN_PROJECT_LAYER_OPACITY..=ori_domain::MAX_PROJECT_LAYER_OPACITY)
                    .contains(opacity)
            {
                return Err(EditorHistoryErrorV1::InvalidCommand);
            }
        }
        Command::AddUnderlay { record } | Command::UpdateUnderlay { record } => {
            let mut document = UnderlayDocumentV1::default();
            document.underlays.push(record.clone());
            validate_underlay_document_v1(&document)
                .map_err(|_| EditorHistoryErrorV1::InvalidCommand)?;
        }
        Command::RemoveVertex { .. }
        | Command::AddEdge { .. }
        | Command::RemoveConnectedVertex { .. }
        | Command::RemoveEdge { .. }
        | Command::SetCuttingAllowed { .. }
        | Command::SetLengthDisplayUnit { .. }
        | Command::ConnectEdgeIntersection { .. }
        | Command::ConnectTJunction { .. }
        | Command::ConnectIntersectionCluster { .. }
        | Command::RemoveBoundaryVertex { .. }
        | Command::RemoveGeometricConstraint { .. }
        | Command::AddAnnotation { .. }
        | Command::UpdateAnnotation { .. }
        | Command::RemoveAnnotation { .. }
        | Command::RemoveUnderlay { .. }
        | Command::UpdateInstructionStepMetadata { .. }
        | Command::RemoveInstructionStep { .. }
        | Command::MoveInstructionStep { .. }
        | Command::CreateLayer { .. }
        | Command::RenameLayer { .. }
        | Command::MoveLayer { .. }
        | Command::DeleteLayer { .. }
        | Command::AssignEdgeToLayer { .. } => {}
    }
    Ok(())
}

fn validate_vertex_finite(vertex: &Vertex) -> Result<(), EditorHistoryErrorV1> {
    if finite_point(vertex.position) {
        Ok(())
    } else {
        Err(EditorHistoryErrorV1::NonFiniteNumber)
    }
}

fn validate_inverse_finite(inverse: &Inverse) -> Result<(), EditorHistoryErrorV1> {
    match inverse {
        Inverse::RestoreMirrorSelection {
            pattern,
            project_layers,
        } => {
            pattern
                .vertices
                .iter()
                .try_for_each(validate_vertex_finite)?;
            validate_project_layer_document_against_pattern_v1(project_layers, pattern)
                .map_err(|_| EditorHistoryErrorV1::InvalidInverse)?;
        }
        Inverse::RestoreStackedFoldDocument {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            ..
        } => validate_stacked_fold_document(
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            EditorHistoryErrorV1::InvalidInverse,
        )?,
        Inverse::RestoreProjectMemo { memo } => {
            if memo.chars().count() > 16_000
                || memo.chars().any(|character| {
                    character.is_control() && !matches!(character, '\n' | '\r' | '\t')
                })
            {
                return Err(EditorHistoryErrorV1::InvalidInverse);
            }
        }
        Inverse::RestoreBeginnerDesignProfile { profile } => {
            if !validate_beginner_design_profile_v1(profile) {
                return Err(EditorHistoryErrorV1::InvalidInverse);
            }
        }
        Inverse::RestoreElementMetadata { metadata, .. } => {
            if let Some(metadata) = metadata {
                ori_domain::validate_element_metadata_v1(metadata)
                    .map_err(|_| EditorHistoryErrorV1::InvalidInverse)?;
            }
        }
        Inverse::Command(command) => validate_command_finite(command)?,
        Inverse::RestoreVertex { vertex, .. } => validate_vertex_finite(vertex)?,
        Inverse::RestoreEdge { .. } | Inverse::RestoreLengthDisplayUnit { .. } => {}
        Inverse::RestorePaperProperties { thickness_mm, .. } => {
            if !thickness_mm.is_finite() {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Inverse::RestoreVertexPositions { vertices } => {
            if vertices.iter().any(|(_, point)| !finite_point(*point)) {
                return Err(EditorHistoryErrorV1::NonFiniteNumber);
            }
        }
        Inverse::RestoreBoundarySplit { new_vertex, .. }
        | Inverse::RestoreEdgeSplit { new_vertex, .. }
        | Inverse::RestoreEdgeIntersection { new_vertex, .. }
        | Inverse::RestoreBoundaryVertexRemoval {
            vertex: new_vertex, ..
        } => validate_vertex_finite(new_vertex)?,
        Inverse::RestoreTJunction { .. }
        | Inverse::RemoveAddedGeometricConstraint { .. }
        | Inverse::RemoveAddedInstructionStep { .. }
        | Inverse::RemoveAppendedInstructionSteps { .. }
        | Inverse::RestoreInstructionStepMetadata { .. }
        | Inverse::RestoreInstructionStepOrder { .. } => {}
        Inverse::RestoreIntersectionCluster { created_vertex, .. } => {
            if let Some((_, vertex)) = created_vertex {
                validate_vertex_finite(vertex)?;
            }
        }
        Inverse::RestoreRemovedGeometricConstraint { record, .. } => {
            validate_constraint_finite(record)?;
        }
        Inverse::RestoreInstructionStepPose { pose, .. } => {
            validate_instruction_pose_finite(pose)?;
        }
        Inverse::RestoreRemovedInstructionStep { step, .. } => {
            validate_instruction_step_finite(step)?;
        }
        Inverse::RestoreDeletedLayer { .. } => {}
    }
    Ok(())
}

fn validate_editor_finite(editor: &EditorState) -> Result<(), EditorHistoryErrorV1> {
    if !editor.paper.thickness_mm.is_finite()
        || editor
            .pattern
            .vertices
            .iter()
            .any(|vertex| !finite_point(vertex.position))
    {
        return Err(EditorHistoryErrorV1::NonFiniteNumber);
    }
    for record in &editor.geometric_constraints.constraints {
        validate_constraint_finite(record)?;
    }
    for step in &editor.instruction_timeline.steps {
        validate_instruction_step_finite(step)?;
    }
    validate_project_layer_document_against_pattern_v1(&editor.project_layers, &editor.pattern)
        .map_err(|_| EditorHistoryErrorV1::InvalidCommand)?;
    ori_domain::validate_annotation_document_v1(&editor.annotations)
        .map_err(|_| EditorHistoryErrorV1::InvalidCommand)?;
    validate_underlay_document_v1(&editor.underlays)
        .map_err(|_| EditorHistoryErrorV1::InvalidCommand)?;
    Ok(())
}

#[derive(Serialize)]
struct EditorDocumentPartsRef<'a> {
    pattern: &'a CreasePattern,
    paper: &'a Paper,
    geometric_constraints: &'a GeometricConstraintDocumentV1,
    instruction_timeline: &'a InstructionTimeline,
    project_layers: &'a ProjectLayerDocumentV1,
    element_metadata: &'a ElementMetadataDocumentV1,
    annotations: &'a AnnotationDocumentV1,
    underlays: &'a UnderlayDocumentV1,
}

fn editor_document_parts_bytes(editor: &EditorState) -> Result<Vec<u8>, EditorHistoryErrorV1> {
    validate_editor_finite(editor)?;
    serde_json::to_vec(&EditorDocumentPartsRef {
        pattern: &editor.pattern,
        paper: &editor.paper,
        geometric_constraints: &editor.geometric_constraints,
        instruction_timeline: &editor.instruction_timeline,
        project_layers: &editor.project_layers,
        element_metadata: &editor.element_metadata,
        annotations: &editor.annotations,
        underlays: &editor.underlays,
    })
    .map_err(|_| EditorHistoryErrorV1::EncodingFailed)
}

fn inverse_bytes(inverse: &Inverse) -> Result<Vec<u8>, EditorHistoryErrorV1> {
    validate_inverse_finite(inverse)?;
    serde_json::to_vec(&inverse_to_wire(inverse)?).map_err(|_| EditorHistoryErrorV1::EncodingFailed)
}

fn vertex_bits_equal(first: &Vertex, second: &Vertex) -> bool {
    first.id == second.id && point_bits_equal(first.position, second.position)
}

fn layer_assignment_matches(
    editor: &EditorState,
    edge: EdgeId,
    expected: Option<EdgeLayerAssignmentV1>,
) -> bool {
    editor
        .explicit_layer_assignment(edge)
        .map(|(_, assignment)| assignment)
        == expected
}

fn layer_assignments_match_edges(
    editor: &EditorState,
    edges: &[EdgeId],
    expected: &[EdgeLayerAssignmentV1],
) -> bool {
    expected
        .iter()
        .all(|assignment| edges.contains(&assignment.edge))
        && edges.iter().all(|edge| {
            layer_assignment_matches(
                editor,
                *edge,
                expected
                    .iter()
                    .find(|assignment| assignment.edge == *edge)
                    .copied(),
            )
        })
}

fn validate_inverse_application(
    editor: &EditorState,
    inverse: &Inverse,
) -> Result<(), EditorHistoryErrorV1> {
    let invalid = || EditorHistoryErrorV1::InvalidInverse;
    match inverse {
        Inverse::RestoreMirrorSelection { .. } => {}
        Inverse::RestoreStackedFoldDocument { .. } => {}
        Inverse::RestoreProjectMemo { .. } => {}
        Inverse::RestoreBeginnerDesignProfile { .. } => {}
        Inverse::RestoreElementMetadata { .. } => {}
        Inverse::Command(_) => {}
        Inverse::RestoreVertex { index, vertex } => {
            if *index > editor.pattern.vertices.len()
                || editor
                    .pattern
                    .vertices
                    .iter()
                    .any(|candidate| candidate.id == vertex.id)
            {
                return Err(invalid());
            }
        }
        Inverse::RestoreEdge {
            index,
            edge,
            layer_assignment,
        } => {
            if *index > editor.pattern.edges.len()
                || editor
                    .pattern
                    .edges
                    .iter()
                    .any(|candidate| candidate.id == edge.id)
                || editor.explicit_layer_assignment(edge.id).is_some()
                || layer_assignment
                    .as_ref()
                    .is_some_and(|(assignment_index, assignment)| {
                        *assignment_index > editor.project_layers.edge_assignments.len()
                            || assignment.edge != edge.id
                    })
            {
                return Err(invalid());
            }
        }
        Inverse::RestorePaperProperties { .. } | Inverse::RestoreLengthDisplayUnit { .. } => {}
        Inverse::RestoreVertexPositions { vertices } => {
            if vertices.len() != editor.pattern.vertices.len()
                || editor
                    .pattern
                    .vertices
                    .iter()
                    .zip(vertices)
                    .any(|(current, (expected, _))| current.id != *expected)
            {
                return Err(invalid());
            }
        }
        Inverse::RestoreBoundarySplit {
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
            new_edge_assignment,
            ..
        }
        | Inverse::RestoreEdgeSplit {
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
            new_edge_assignment,
        } => {
            let mut edges = editor.pattern.edges.clone();
            if edges.get(*new_edge_index) != Some(new_edge) {
                return Err(invalid());
            }
            edges.remove(*new_edge_index);
            if edges
                .get(*original_edge_index)
                .is_none_or(|edge| edge.id != original_edge.id)
            {
                return Err(invalid());
            }
            let mut vertices = editor.pattern.vertices.clone();
            if vertices
                .get(*new_vertex_index)
                .is_none_or(|vertex| !vertex_bits_equal(vertex, new_vertex))
            {
                return Err(invalid());
            }
            vertices.remove(*new_vertex_index);
            if !layer_assignment_matches(editor, new_edge.id, *new_edge_assignment) {
                return Err(invalid());
            }
        }
        Inverse::RestoreEdgeIntersection {
            original_edges,
            new_edges,
            new_vertex_index,
            new_vertex,
            new_edge_assignments,
        } => {
            let mut edges = editor.pattern.edges.clone();
            for (index, expected) in new_edges.iter().rev() {
                if edges.get(*index) != Some(expected) {
                    return Err(invalid());
                }
                edges.remove(*index);
            }
            for (index, expected) in original_edges {
                if edges.get(*index).is_none_or(|edge| edge.id != expected.id) {
                    return Err(invalid());
                }
                edges[*index] = expected.clone();
            }
            if editor
                .pattern
                .vertices
                .get(*new_vertex_index)
                .is_none_or(|vertex| !vertex_bits_equal(vertex, new_vertex))
            {
                return Err(invalid());
            }
            let generated = [new_edges[0].1.id, new_edges[1].1.id];
            if !layer_assignments_match_edges(editor, &generated, new_edge_assignments) {
                return Err(invalid());
            }
        }
        Inverse::RestoreTJunction {
            original_edge_index,
            original_edge,
            new_edge_index,
            new_edge,
            new_edge_assignment,
            ..
        } => {
            let mut edges = editor.pattern.edges.clone();
            if edges.get(*new_edge_index) != Some(new_edge) {
                return Err(invalid());
            }
            edges.remove(*new_edge_index);
            if edges
                .get(*original_edge_index)
                .is_none_or(|edge| edge.id != original_edge.id)
            {
                return Err(invalid());
            }
            if !layer_assignment_matches(editor, new_edge.id, *new_edge_assignment) {
                return Err(invalid());
            }
        }
        Inverse::RestoreIntersectionCluster {
            original_edges,
            inserted_edges,
            created_vertex,
            junction_vertex,
            new_edge_assignments,
            ..
        } => {
            if !editor
                .pattern
                .vertices
                .iter()
                .any(|vertex| vertex.id == *junction_vertex)
            {
                return Err(invalid());
            }
            let mut edges = editor.pattern.edges.clone();
            for (index, expected) in inserted_edges.iter().rev() {
                if edges.get(*index) != Some(expected) {
                    return Err(invalid());
                }
                edges.remove(*index);
            }
            for (index, expected) in original_edges {
                if edges.get(*index).is_none_or(|edge| edge.id != expected.id) {
                    return Err(invalid());
                }
                edges[*index] = expected.clone();
            }
            if let Some((index, expected)) = created_vertex
                && editor
                    .pattern
                    .vertices
                    .get(*index)
                    .is_none_or(|vertex| !vertex_bits_equal(vertex, expected))
            {
                return Err(invalid());
            }
            let generated = inserted_edges
                .iter()
                .map(|(_, edge)| edge.id)
                .collect::<Vec<_>>();
            if !layer_assignments_match_edges(editor, &generated, new_edge_assignments) {
                return Err(invalid());
            }
        }
        Inverse::RestoreBoundaryVertexRemoval {
            boundary_index,
            vertex_index,
            kept_edge_index,
            kept_edge,
            removed_edge_index,
            removed_edge,
            removed_edge_assignment,
            ..
        } => {
            let current_kept_index = if removed_edge_index < kept_edge_index {
                kept_edge_index.checked_sub(1).ok_or_else(invalid)?
            } else {
                *kept_edge_index
            };
            if editor
                .pattern
                .edges
                .get(current_kept_index)
                .is_none_or(|edge| edge.id != kept_edge.id)
                || *removed_edge_index > editor.pattern.edges.len()
                || *vertex_index > editor.pattern.vertices.len()
                || *boundary_index > editor.paper.boundary_vertices.len()
                || editor.explicit_layer_assignment(removed_edge.id).is_some()
                || removed_edge_assignment
                    .as_ref()
                    .is_some_and(|(assignment_index, assignment)| {
                        *assignment_index > editor.project_layers.edge_assignments.len()
                            || assignment.edge != removed_edge.id
                    })
            {
                return Err(invalid());
            }
            let mut edges = editor.pattern.edges.clone();
            edges.insert(*removed_edge_index, removed_edge.clone());
            if *kept_edge_index >= edges.len() {
                return Err(invalid());
            }
        }
        Inverse::RemoveAddedGeometricConstraint { id } => {
            if !editor
                .geometric_constraints
                .constraints
                .iter()
                .any(|record| record.id == *id)
            {
                return Err(invalid());
            }
        }
        Inverse::RestoreRemovedGeometricConstraint { index, .. } => {
            if *index > editor.geometric_constraints.constraints.len() {
                return Err(invalid());
            }
        }
        Inverse::RemoveAddedInstructionStep { step_id } => {
            if !editor
                .instruction_timeline
                .steps
                .iter()
                .any(|step| step.id == *step_id)
            {
                return Err(invalid());
            }
        }
        Inverse::RemoveAppendedInstructionSteps { step_ids } => {
            if step_ids.is_empty() || step_ids.len() > editor.instruction_timeline.steps.len() {
                return Err(invalid());
            }
            let suffix_start = editor.instruction_timeline.steps.len() - step_ids.len();
            if editor.instruction_timeline.steps[suffix_start..]
                .iter()
                .map(|step| step.id)
                .ne(step_ids.iter().copied())
            {
                return Err(invalid());
            }
        }
        Inverse::RestoreInstructionStepMetadata { step_id, .. }
        | Inverse::RestoreInstructionStepPose { step_id, .. }
        | Inverse::RestoreInstructionStepOrder { step_id, .. } => {
            if !editor
                .instruction_timeline
                .steps
                .iter()
                .any(|step| step.id == *step_id)
            {
                return Err(invalid());
            }
            if let Inverse::RestoreInstructionStepOrder { previous_index, .. } = inverse
                && *previous_index >= editor.instruction_timeline.steps.len()
            {
                return Err(invalid());
            }
        }
        Inverse::RestoreRemovedInstructionStep { index, step } => {
            if *index > editor.instruction_timeline.steps.len()
                || editor
                    .instruction_timeline
                    .steps
                    .iter()
                    .any(|candidate| candidate.id == step.id)
            {
                return Err(invalid());
            }
        }
        Inverse::RestoreDeletedLayer {
            index,
            layer,
            assignments,
        } => {
            let mut candidate = editor.project_layers.clone();
            if assignments.len() > MAX_LAYER_EDGE_ASSIGNMENTS
                || *index > candidate.layers.len()
                || candidate.layers.iter().any(|record| record.id == layer.id)
            {
                return Err(invalid());
            }
            candidate.layers.insert(*index, layer.clone());
            for (assignment_index, assignment) in assignments {
                if *assignment_index > candidate.edge_assignments.len()
                    || candidate
                        .edge_assignments
                        .iter()
                        .any(|current| current.edge == assignment.edge)
                {
                    return Err(invalid());
                }
                candidate
                    .edge_assignments
                    .insert(*assignment_index, *assignment);
            }
            if validate_project_layer_document_against_pattern_v1(&candidate, &editor.pattern)
                .is_err()
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

fn apply_persisted_inverse(
    editor: &mut EditorState,
    inverse: &Inverse,
) -> Result<(), EditorHistoryErrorV1> {
    validate_inverse_finite(inverse)?;
    validate_inverse_application(editor, inverse)?;
    editor
        .apply_inverse(inverse)
        .map_err(|_| EditorHistoryErrorV1::InvalidInverse)?;
    validate_project_layer_document_against_pattern_v1(&editor.project_layers, &editor.pattern)
        .map_err(|_| EditorHistoryErrorV1::InvalidInverse)
}

fn replay_forward(
    editor: &mut EditorState,
    forward: Command,
) -> Result<HistoryEntry, EditorHistoryErrorV1> {
    validate_command_finite(&forward)?;
    let geometry_before = forward
        .may_change_kinematic_geometry()
        .then(|| editor.fold_model_fingerprint_v1());
    let inverse = editor
        .apply(&forward)
        .map_err(|_| EditorHistoryErrorV1::InvalidCommand)?;
    validate_project_layer_document_against_pattern_v1(&editor.project_layers, &editor.pattern)
        .map_err(|_| EditorHistoryErrorV1::InvalidCommand)?;
    let applied_pose =
        if geometry_before.is_some_and(|before| before != editor.fold_model_fingerprint_v1()) {
            AppliedPoseHistoryTransition::Restore {
                before: None,
                after: None,
            }
        } else {
            AppliedPoseHistoryTransition::PreserveCurrent
        };
    Ok(HistoryEntry {
        forward,
        inverse,
        applied_pose,
    })
}

fn inverse_is_exact(first: &Inverse, second: &Inverse) -> Result<bool, EditorHistoryErrorV1> {
    Ok(inverse_bytes(first)? == inverse_bytes(second)?)
}

impl EditorState {
    /// Exports document-history deltas without persisting runtime pose values.
    pub fn export_history_v1(
        &self,
        project_id: ProjectId,
    ) -> Result<EditorHistoryV1, EditorHistoryErrorV1> {
        validate_editor_finite(self)?;
        if project_id.canonical_bytes() == [0; 16] {
            return Err(EditorHistoryErrorV1::NilProjectId);
        }
        let history = EditorHistoryV1 {
            schema_version: EDITOR_HISTORY_SCHEMA_VERSION_V1,
            project_id,
            history_entry_limit: index_to_wire(self.history_entry_limit)?,
            undo_stack: self
                .undo_stack
                .iter()
                .map(entry_to_wire)
                .collect::<Result<Vec<_>, _>>()?,
            redo_stack: self
                .redo_stack
                .iter()
                .map(entry_to_wire)
                .collect::<Result<Vec<_>, _>>()?,
        };
        history.validate_shape()?;
        Ok(history)
    }

    /// Restores an editor only after every untrusted history edge has been
    /// checked, replayed, and rebound to the exact supplied current document.
    ///
    /// Revision and runtime pose intentionally restart at zero/`None`.
    pub fn with_document_parts_and_history_v1(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        history: EditorHistoryV1,
    ) -> Result<Self, EditorHistoryErrorV1> {
        Self::with_document_parts_layers_and_history_v1(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            ProjectLayerDocumentV1::default(),
            history,
        )
    }

    /// Restores history bound to every persisted editor-owned document part,
    /// including the strict LIN-004 layer document.
    pub fn with_document_parts_layers_and_history_v1(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        history: EditorHistoryV1,
    ) -> Result<Self, EditorHistoryErrorV1> {
        Self::with_all_document_parts_and_history_v1(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            ElementMetadataDocumentV1::default(),
            history,
        )
    }

    pub fn with_all_document_parts_and_history_v1(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
        history: EditorHistoryV1,
    ) -> Result<Self, EditorHistoryErrorV1> {
        Self::with_all_document_parts_memo_and_history_v1(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            element_metadata,
            String::new(),
            history,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_all_document_parts_memo_and_history_v1(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
        project_memo: String,
        history: EditorHistoryV1,
    ) -> Result<Self, EditorHistoryErrorV1> {
        Self::with_all_document_parts_annotations_memo_and_history_v1(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            element_metadata,
            AnnotationDocumentV1::default(),
            project_memo,
            history,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_all_document_parts_annotations_memo_and_history_v1(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
        annotations: AnnotationDocumentV1,
        project_memo: String,
        history: EditorHistoryV1,
    ) -> Result<Self, EditorHistoryErrorV1> {
        Self::with_all_document_parts_annotations_underlays_memo_and_history_v1(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            element_metadata,
            annotations,
            UnderlayDocumentV1::default(),
            project_memo,
            history,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_all_document_parts_annotations_underlays_memo_and_history_v1(
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        geometric_constraints: GeometricConstraintDocumentV1,
        project_layers: ProjectLayerDocumentV1,
        element_metadata: ElementMetadataDocumentV1,
        annotations: AnnotationDocumentV1,
        underlays: UnderlayDocumentV1,
        project_memo: String,
        history: EditorHistoryV1,
    ) -> Result<Self, EditorHistoryErrorV1> {
        let limit = history.validate_shape()?;
        let mut current = Self::with_all_document_parts_annotations_underlays_and_memo(
            pattern,
            paper,
            instruction_timeline,
            geometric_constraints,
            project_layers,
            element_metadata,
            annotations,
            underlays,
            project_memo,
        );
        validate_editor_finite(&current)?;
        let expected_current = editor_document_parts_bytes(&current)?;

        let undo_wire = history
            .undo_stack
            .into_iter()
            .map(entry_from_wire)
            .collect::<Result<Vec<_>, _>>()?;
        let redo_wire = history
            .redo_stack
            .into_iter()
            .map(entry_from_wire)
            .collect::<Result<Vec<_>, _>>()?;

        let mut base = current.clone();
        for (_, inverse) in undo_wire.iter().rev() {
            apply_persisted_inverse(&mut base, inverse)?;
        }

        let mut rebuilt = base;
        let mut undo_stack = Vec::with_capacity(undo_wire.len());
        for (forward, expected_inverse) in undo_wire {
            let generated = replay_forward(&mut rebuilt, forward)?;
            if !inverse_is_exact(&generated.inverse, &expected_inverse)? {
                return Err(EditorHistoryErrorV1::InverseMismatch);
            }
            undo_stack.push(generated);
        }
        if editor_document_parts_bytes(&rebuilt)? != expected_current {
            return Err(EditorHistoryErrorV1::CurrentDocumentMismatch);
        }

        let mut redo_cursor = current.clone();
        let mut redo_application_order = Vec::with_capacity(redo_wire.len());
        for (forward, expected_inverse) in redo_wire.into_iter().rev() {
            let generated = replay_forward(&mut redo_cursor, forward)?;
            if !inverse_is_exact(&generated.inverse, &expected_inverse)? {
                return Err(EditorHistoryErrorV1::InverseMismatch);
            }
            redo_application_order.push(generated);
        }
        redo_application_order.reverse();

        current.undo_stack = undo_stack;
        current.redo_stack = redo_application_order;
        current.history_entry_limit = limit;
        current.revision = 0;
        current.current_applied_pose = None;
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        ConstraintId, FaceId, GeometricConstraintKindV1, InstructionHingeAngle,
        InstructionPoseModel,
    };
    use serde_json::{Value, json};

    use super::*;

    fn instruction_pose() -> InstructionPose {
        InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: "0".repeat(64),
            fixed_face: Some(FaceId::new()),
            hinge_angles: vec![InstructionHingeAngle {
                edge: EdgeId::new(),
                angle_degrees: 45.0,
            }],
        }
    }

    fn instruction_step() -> InstructionStep {
        InstructionStep {
            id: InstructionStepId::new(),
            title: "step".to_owned(),
            description: "description".to_owned(),
            caution: "caution".to_owned(),
            duration_ms: 1_000,
            visual: Default::default(),
            pose: instruction_pose(),
        }
    }

    fn declarative_instruction_step(title: &str) -> InstructionStep {
        InstructionStep {
            id: InstructionStepId::new(),
            title: title.to_owned(),
            description: "description-only".to_owned(),
            caution: "no physical command".to_owned(),
            duration_ms: 1_000,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint: "0".repeat(64),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        }
    }

    fn constraint_record() -> GeometricConstraintRecordV1 {
        GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: EdgeId::new(),
            },
        }
    }

    fn all_commands() -> Vec<Command> {
        let vertex = VertexId::new();
        let other_vertex = VertexId::new();
        let edge = EdgeId::new();
        let other_edge = EdgeId::new();
        let generated_edge = EdgeId::new();
        let step = instruction_step();
        let layer = LayerId::new();
        let layer_record = LayerRecordV1 {
            id: layer,
            name: "Details".to_owned(),
            content_kind: ori_domain::LayerContentKindV1::CreasePattern,
            visible: true,
            locked: false,
            opacity: 1.0,
        };
        vec![
            Command::AddVertex {
                id: vertex,
                position: Point2::new(-0.0, 1.0),
            },
            Command::MoveVertex {
                id: vertex,
                position: Point2::new(2.0, 3.0),
            },
            Command::RemoveVertex { id: vertex },
            Command::AddEdge {
                id: edge,
                start: vertex,
                end: other_vertex,
                kind: EdgeKind::Mountain,
            },
            Command::AddConnectedVertex {
                vertex_id: other_vertex,
                position: Point2::new(4.0, 5.0),
                edge_id: other_edge,
                start: vertex,
                kind: EdgeKind::Valley,
            },
            Command::RemoveConnectedVertex {
                vertex_id: other_vertex,
                edge_id: other_edge,
            },
            Command::RemoveEdge { id: edge },
            Command::SetCuttingAllowed { allowed: true },
            Command::UpdatePaperProperties {
                thickness_mm: 0.1,
                front_color: RgbaColor::opaque(1, 2, 3),
                back_color: RgbaColor::opaque(4, 5, 6),
                front_texture_asset: None,
                back_texture_asset: None,
                cutting_allowed: true,
            },
            Command::SetLengthDisplayUnit {
                unit: LengthDisplayUnit::PaperEdgeRatio {
                    reference_edge: edge,
                },
            },
            Command::ResizeRectangularPaper {
                width_mm: 200.0,
                height_mm: 300.0,
            },
            Command::SplitEdge {
                edge,
                new_vertex: other_vertex,
                new_edge: generated_edge,
                fraction: 0.5,
            },
            Command::ConnectEdgeIntersection {
                first_edge: edge,
                second_edge: other_edge,
                new_vertex: vertex,
                first_new_edge: generated_edge,
                second_new_edge: EdgeId::new(),
            },
            Command::ConnectTJunction {
                first_edge: edge,
                second_edge: other_edge,
                new_edge: generated_edge,
            },
            Command::ConnectIntersectionCluster {
                junction: JunctionVertexIntent::Create { id: vertex },
                targets: vec![
                    IntersectionEdgeTarget {
                        edge,
                        new_edge: Some(generated_edge),
                    },
                    IntersectionEdgeTarget {
                        edge: other_edge,
                        new_edge: None,
                    },
                    IntersectionEdgeTarget {
                        edge: EdgeId::new(),
                        new_edge: Some(EdgeId::new()),
                    },
                ],
            },
            Command::SplitBoundaryEdge {
                edge,
                new_vertex: other_vertex,
                new_edge: generated_edge,
                fraction: 0.25,
            },
            Command::RemoveBoundaryVertex { vertex },
            Command::AddGeometricConstraint {
                record: constraint_record(),
            },
            Command::RemoveGeometricConstraint {
                id: ConstraintId::new(),
            },
            Command::AddInstructionStep { step: step.clone() },
            Command::AppendInstructionSteps {
                steps: vec![step.clone(), instruction_step()],
            },
            Command::UpdateInstructionStepMetadata {
                step_id: step.id,
                title: "updated".to_owned(),
                description: "updated description".to_owned(),
                caution: "updated caution".to_owned(),
                duration_ms: 2_000,
                visual: Default::default(),
            },
            Command::ReplaceInstructionStepPose {
                step_id: step.id,
                pose: instruction_pose(),
            },
            Command::RemoveInstructionStep { step_id: step.id },
            Command::MoveInstructionStep {
                step_id: step.id,
                target_index: 7,
            },
            Command::CreateLayer {
                layer: layer_record,
                target_index: 1,
            },
            Command::RenameLayer {
                layer,
                name: "Renamed".to_owned(),
            },
            Command::UpdateLayerPresentation {
                layer,
                visible: false,
                locked: true,
                opacity: 0.35,
            },
            Command::MoveLayer {
                layer,
                target_index: 0,
            },
            Command::DeleteLayer { layer },
            Command::AssignEdgeToLayer { edge, layer },
        ]
    }

    fn command_tag(command: &CommandV1) -> String {
        serde_json::to_value(command)
            .expect("serialize command")
            .get("kind")
            .and_then(Value::as_str)
            .expect("command kind")
            .to_owned()
    }

    #[test]
    fn every_command_variant_has_an_exact_v1_wire_tag_and_round_trip() {
        let commands = all_commands();
        let expected_tags = [
            "add_vertex",
            "move_vertex",
            "remove_vertex",
            "add_edge",
            "add_connected_vertex",
            "remove_connected_vertex",
            "remove_edge",
            "set_cutting_allowed",
            "update_paper_properties",
            "set_length_display_unit",
            "resize_rectangular_paper",
            "split_edge",
            "connect_edge_intersection",
            "connect_t_junction",
            "connect_intersection_cluster",
            "split_boundary_edge",
            "remove_boundary_vertex",
            "add_geometric_constraint",
            "remove_geometric_constraint",
            "add_instruction_step",
            "append_instruction_steps",
            "update_instruction_step_metadata",
            "replace_instruction_step_pose",
            "remove_instruction_step",
            "move_instruction_step",
            "create_layer",
            "rename_layer",
            "update_layer_presentation",
            "move_layer",
            "delete_layer",
            "assign_edge_to_layer",
        ];
        assert_eq!(commands.len(), expected_tags.len());

        for (command, expected_tag) in commands.into_iter().zip(expected_tags) {
            let wire = command_to_wire(&command).expect("convert command to V1");
            assert_eq!(command_tag(&wire), expected_tag);
            let encoded = serde_json::to_vec(&wire).expect("serialize command V1");
            let decoded: CommandV1 =
                serde_json::from_slice(&encoded).expect("deserialize command V1");
            assert_eq!(
                command_from_wire(decoded).expect("convert command from V1"),
                command
            );
        }
    }

    fn indexed_vertex(index: usize, id: VertexId, x: f64) -> (usize, Vertex) {
        (
            index,
            Vertex {
                id,
                position: Point2::new(x, x + 1.0),
            },
        )
    }

    fn indexed_edge(index: usize, id: EdgeId, start: VertexId, end: VertexId) -> (usize, Edge) {
        (
            index,
            Edge {
                id,
                start,
                end,
                kind: EdgeKind::Valley,
            },
        )
    }

    fn all_inverses() -> Vec<Inverse> {
        let vertex = VertexId::new();
        let other_vertex = VertexId::new();
        let third_vertex = VertexId::new();
        let fourth_vertex = VertexId::new();
        let edge = EdgeId::new();
        let other_edge = EdgeId::new();
        let third_edge = EdgeId::new();
        let fourth_edge = EdgeId::new();
        let layer = LayerId::new();
        let assignment = |edge| EdgeLayerAssignmentV1 { edge, layer };
        let step = instruction_step();
        vec![
            Inverse::Command(Command::RemoveVertex { id: vertex }),
            Inverse::RestoreVertex {
                index: 1,
                vertex: indexed_vertex(1, vertex, -0.0).1,
            },
            Inverse::RestoreEdge {
                index: 2,
                edge: indexed_edge(2, edge, vertex, other_vertex).1,
                layer_assignment: Some((1, assignment(edge))),
            },
            Inverse::RestorePaperProperties {
                thickness_mm: 0.1,
                front_color: RgbaColor::opaque(1, 2, 3),
                back_color: RgbaColor::opaque(4, 5, 6),
                front_texture_asset: None,
                back_texture_asset: None,
                cutting_allowed: false,
            },
            Inverse::RestoreLengthDisplayUnit {
                unit: LengthDisplayUnit::Millimeter,
            },
            Inverse::RestoreVertexPositions {
                vertices: vec![
                    (vertex, Point2::new(-0.0, 1.0)),
                    (other_vertex, Point2::new(2.0, 3.0)),
                ],
            },
            Inverse::RestoreBoundarySplit {
                boundary_vertices: vec![vertex, other_vertex, third_vertex],
                original_edge_index: 0,
                original_edge: indexed_edge(0, edge, vertex, other_vertex).1,
                new_vertex_index: 3,
                new_vertex: indexed_vertex(3, fourth_vertex, 4.0).1,
                new_edge_index: 1,
                new_edge: indexed_edge(1, other_edge, fourth_vertex, other_vertex).1,
                new_edge_assignment: Some(assignment(other_edge)),
            },
            Inverse::RestoreEdgeSplit {
                original_edge_index: 0,
                original_edge: indexed_edge(0, edge, vertex, other_vertex).1,
                new_vertex_index: 3,
                new_vertex: indexed_vertex(3, fourth_vertex, 4.0).1,
                new_edge_index: 1,
                new_edge: indexed_edge(1, other_edge, fourth_vertex, other_vertex).1,
                new_edge_assignment: Some(assignment(other_edge)),
            },
            Inverse::RestoreEdgeIntersection {
                original_edges: [
                    indexed_edge(0, edge, vertex, other_vertex),
                    indexed_edge(2, other_edge, third_vertex, fourth_vertex),
                ],
                new_edges: [
                    indexed_edge(1, third_edge, fourth_vertex, other_vertex),
                    indexed_edge(3, fourth_edge, fourth_vertex, third_vertex),
                ],
                new_vertex_index: 4,
                new_vertex: indexed_vertex(4, fourth_vertex, 4.0).1,
                new_edge_assignments: vec![assignment(third_edge), assignment(fourth_edge)],
            },
            Inverse::RestoreTJunction {
                original_edge_index: 0,
                original_edge: indexed_edge(0, edge, vertex, other_vertex).1,
                new_edge_index: 1,
                new_edge: indexed_edge(1, other_edge, third_vertex, other_vertex).1,
                boundary_vertices: Some(vec![vertex, other_vertex, third_vertex]),
                changed_vertices: [vertex, other_vertex, third_vertex, fourth_vertex],
                changed_edges: [edge, other_edge, third_edge],
                new_edge_assignment: Some(assignment(other_edge)),
            },
            Inverse::RestoreIntersectionCluster {
                original_boundary_vertices: None,
                original_edges: vec![indexed_edge(0, edge, vertex, other_vertex)],
                inserted_edges: vec![indexed_edge(1, other_edge, fourth_vertex, other_vertex)],
                created_vertex: Some(indexed_vertex(4, fourth_vertex, 4.0)),
                junction_vertex: fourth_vertex,
                changed_vertices: vec![vertex, other_vertex, fourth_vertex],
                changed_edges: vec![edge, other_edge],
                new_edge_assignments: vec![assignment(other_edge)],
            },
            Inverse::RestoreBoundaryVertexRemoval {
                boundary_index: 1,
                vertex_index: 1,
                vertex: indexed_vertex(1, vertex, 1.0).1,
                kept_edge_index: 0,
                kept_edge: indexed_edge(0, edge, other_vertex, vertex).1,
                removed_edge_index: 1,
                removed_edge: indexed_edge(1, other_edge, vertex, third_vertex).1,
                previous_vertex: other_vertex,
                next_vertex: third_vertex,
                removed_edge_assignment: Some((1, assignment(other_edge))),
            },
            Inverse::RemoveAddedGeometricConstraint {
                id: ConstraintId::new(),
            },
            Inverse::RestoreRemovedGeometricConstraint {
                index: 3,
                record: constraint_record(),
            },
            Inverse::RemoveAddedInstructionStep { step_id: step.id },
            Inverse::RemoveAppendedInstructionSteps {
                step_ids: vec![step.id, InstructionStepId::new()],
            },
            Inverse::RestoreInstructionStepMetadata {
                step_id: step.id,
                title: "old".to_owned(),
                description: "old description".to_owned(),
                caution: "old caution".to_owned(),
                duration_ms: 1_000,
                visual: Default::default(),
            },
            Inverse::RestoreInstructionStepPose {
                step_id: step.id,
                pose: instruction_pose(),
            },
            Inverse::RestoreRemovedInstructionStep {
                index: 4,
                step: step.clone(),
            },
            Inverse::RestoreInstructionStepOrder {
                step_id: step.id,
                previous_index: 5,
            },
            Inverse::RestoreDeletedLayer {
                index: 1,
                layer: LayerRecordV1 {
                    id: layer,
                    name: "Deleted".to_owned(),
                    content_kind: ori_domain::LayerContentKindV1::CreasePattern,
                    visible: true,
                    locked: false,
                    opacity: 1.0,
                },
                assignments: vec![(1, assignment(edge))],
            },
        ]
    }

    fn inverse_tag(inverse: &InverseV1) -> String {
        serde_json::to_value(inverse)
            .expect("serialize inverse")
            .get("kind")
            .and_then(Value::as_str)
            .expect("inverse kind")
            .to_owned()
    }

    #[test]
    fn every_inverse_variant_has_an_exact_v1_wire_tag_and_round_trip() {
        let inverses = all_inverses();
        let expected_tags = [
            "command",
            "restore_vertex",
            "restore_edge",
            "restore_paper_properties",
            "restore_length_display_unit",
            "restore_vertex_positions",
            "restore_boundary_split",
            "restore_edge_split",
            "restore_edge_intersection",
            "restore_t_junction",
            "restore_intersection_cluster",
            "restore_boundary_vertex_removal",
            "remove_added_geometric_constraint",
            "restore_removed_geometric_constraint",
            "remove_added_instruction_step",
            "remove_appended_instruction_steps",
            "restore_instruction_step_metadata",
            "restore_instruction_step_pose",
            "restore_removed_instruction_step",
            "restore_instruction_step_order",
            "restore_deleted_layer",
        ];
        assert_eq!(inverses.len(), expected_tags.len());

        for (inverse, expected_tag) in inverses.into_iter().zip(expected_tags) {
            let wire = inverse_to_wire(&inverse).expect("convert inverse to V1");
            assert_eq!(inverse_tag(&wire), expected_tag);
            let encoded = serde_json::to_vec(&wire).expect("serialize inverse V1");
            let decoded: InverseV1 =
                serde_json::from_slice(&encoded).expect("deserialize inverse V1");
            assert_eq!(
                inverse_from_wire(decoded).expect("convert inverse from V1"),
                inverse
            );
        }
    }

    #[test]
    fn public_history_type_is_owned_deserializable_and_serializable() {
        fn require_owned_deserialize<T: serde::de::DeserializeOwned>() {}
        fn require_serialize<T: Serialize>() {}
        fn require_clone_and_partial_eq<T: Clone + PartialEq>() {}

        require_owned_deserialize::<EditorHistoryV1>();
        require_serialize::<EditorHistoryV1>();
        require_clone_and_partial_eq::<EditorHistoryV1>();
    }

    fn dummy_wire_entry() -> HistoryEntryV1 {
        HistoryEntryV1 {
            forward: CommandV1::AddVertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 2.0),
            },
            inverse: InverseV1::Command {
                command: CommandV1::RemoveVertex {
                    id: VertexId::new(),
                },
            },
        }
    }

    fn history_with(
        limit: u32,
        undo_stack: Vec<HistoryEntryV1>,
        redo_stack: Vec<HistoryEntryV1>,
    ) -> EditorHistoryV1 {
        EditorHistoryV1 {
            schema_version: EDITOR_HISTORY_SCHEMA_VERSION_V1,
            project_id: ProjectId::new(),
            history_entry_limit: limit,
            undo_stack,
            redo_stack,
        }
    }

    #[test]
    fn history_wire_rejects_unknown_fields_at_every_owned_envelope() {
        let history = history_with(128, vec![dummy_wire_entry()], Vec::new());
        let mut top_level = serde_json::to_value(&history).expect("history JSON");
        top_level
            .as_object_mut()
            .expect("history object")
            .insert("unexpected".to_owned(), Value::Bool(true));
        assert!(serde_json::from_value::<EditorHistoryV1>(top_level).is_err());

        let mut entry = serde_json::to_value(&history).expect("history JSON");
        entry["undo_stack"][0]
            .as_object_mut()
            .expect("entry object")
            .insert("unexpected".to_owned(), Value::Bool(true));
        assert!(serde_json::from_value::<EditorHistoryV1>(entry).is_err());

        let mut command = serde_json::to_value(&history).expect("history JSON");
        command["undo_stack"][0]["forward"]
            .as_object_mut()
            .expect("command object")
            .insert("unexpected".to_owned(), Value::Bool(true));
        assert!(serde_json::from_value::<EditorHistoryV1>(command).is_err());
    }

    #[test]
    fn history_shape_accepts_each_stack_at_128_and_rejects_every_boundary_overrun() {
        let entry = dummy_wire_entry();
        let maximum = history_with(
            128,
            vec![entry.clone(); MAX_EDITOR_HISTORY_ENTRIES],
            vec![entry.clone(); MAX_EDITOR_HISTORY_ENTRIES],
        );
        assert_eq!(maximum.validate_shape(), Ok(MAX_EDITOR_HISTORY_ENTRIES));

        assert_eq!(
            history_with(0, Vec::new(), Vec::new()).validate_shape(),
            Err(EditorHistoryErrorV1::EntryLimitOutOfRange)
        );
        assert_eq!(
            history_with(129, Vec::new(), Vec::new()).validate_shape(),
            Err(EditorHistoryErrorV1::EntryLimitOutOfRange)
        );
        assert_eq!(
            history_with(128, vec![entry.clone(); 129], Vec::new()).validate_shape(),
            Err(EditorHistoryErrorV1::TooManyUndoEntries)
        );
        assert_eq!(
            history_with(128, Vec::new(), vec![entry; 129]).validate_shape(),
            Err(EditorHistoryErrorV1::TooManyRedoEntries)
        );
    }

    fn restore(
        editor: &EditorState,
        history: EditorHistoryV1,
    ) -> Result<EditorState, EditorHistoryErrorV1> {
        EditorState::with_document_parts_and_history_v1(
            editor.pattern.clone(),
            editor.paper.clone(),
            editor.instruction_timeline.clone(),
            editor.geometric_constraints.clone(),
            history,
        )
    }

    #[test]
    fn stacked_fold_document_history_round_trips_and_rejects_malformed_target() {
        let sheet = crate::create_rectangular_sheet(80.0, 60.0, false).unwrap();
        let (source_pattern, mut paper) = sheet.into_parts();
        paper.thickness_mm = 0.0;
        let mut target_pattern = source_pattern.clone();
        let hinge = EdgeId::new();
        target_pattern.edges.push(Edge {
            id: hinge,
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let timeline = InstructionTimeline {
            steps: vec![InstructionStep {
                id: InstructionStepId::new(),
                title: "fold".to_owned(),
                description: String::new(),
                caution: String::new(),
                duration_ms: ori_domain::MIN_INSTRUCTION_DURATION_MS,
                visual: InstructionVisual::default(),
                pose: InstructionPose {
                    model: ori_domain::InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint:
                        crate::fold_model_fingerprint::fold_model_fingerprint_v1(
                            &target_pattern,
                            &paper,
                        ),
                    fixed_face: Some(FaceId::new()),
                    hinge_angles: vec![ori_domain::InstructionHingeAngle {
                        edge: hinge,
                        angle_degrees: 90.0,
                    }],
                },
            }],
        };
        let mut editor = EditorState::with_paper(source_pattern.clone(), paper.clone());
        editor
            .execute(
                0,
                Command::ApplyStackedFoldDocument {
                    pattern: target_pattern.clone(),
                    paper: paper.clone(),
                    instruction_timeline: timeline.clone(),
                    project_layers: ProjectLayerDocumentV1::default(),
                },
            )
            .unwrap();
        let history = editor.export_history_v1(ProjectId::new()).unwrap();
        let mut reopened = restore(&editor, history).unwrap();
        reopened.undo(0).unwrap();
        assert_eq!(reopened.pattern(), &source_pattern);
        reopened.redo(1).unwrap();
        assert_eq!(reopened.pattern(), &target_pattern);
        assert_eq!(reopened.instruction_timeline(), &timeline);

        let mut malformed = target_pattern;
        malformed.vertices[0].position.x = f64::NAN;
        assert_eq!(
            editor.execute(
                1,
                Command::ApplyStackedFoldDocument {
                    pattern: malformed,
                    paper,
                    instruction_timeline: timeline,
                    project_layers: ProjectLayerDocumentV1::default(),
                },
            ),
            Err(CommandError::InvalidStackedFoldDocument)
        );
        assert_eq!(editor.revision(), 1);
    }

    #[test]
    fn real_undo_and_redo_stacks_reopen_at_revision_zero_with_limit_and_order() {
        let first = VertexId::new();
        let second = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .set_history_entry_limit(7)
            .expect("set fixture limit");
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: first,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("add first vertex");
        editor
            .execute(
                1,
                Command::AddVertex {
                    id: second,
                    position: Point2::new(3.0, 4.0),
                },
            )
            .expect("add second vertex");
        editor.undo(2).expect("create redo stack");

        let project_id = ProjectId::new();
        let history = editor
            .export_history_v1(project_id)
            .expect("export history");
        assert_eq!(history.project_id(), project_id);
        assert_eq!(history.history_entry_limit(), 7);
        let reopened = restore(&editor, history).expect("restore history");
        assert_eq!(reopened.revision(), 0);
        assert_eq!(reopened.history_entry_limit(), 7);
        assert!(reopened.can_undo());
        assert!(reopened.can_redo());

        let mut undo = reopened.clone();
        undo.undo(0).expect("undo reopened first vertex");
        assert!(undo.pattern().vertices.is_empty());

        let mut redo = reopened;
        redo.redo(0).expect("redo reopened second vertex");
        assert_eq!(
            redo.pattern()
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
    }

    #[test]
    fn declarative_batch_append_history_reopens_as_one_undoable_edit() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let steps = vec![
            declarative_instruction_step("Technique"),
            declarative_instruction_step("Operation"),
        ];
        editor
            .execute(
                0,
                Command::AppendInstructionSteps {
                    steps: steps.clone(),
                },
            )
            .expect("append declarative steps");
        let project_id = ProjectId::new();
        let history = editor
            .export_history_v1(project_id)
            .expect("export append history");
        let mut reopened = restore(&editor, history).expect("restore append history");

        assert_eq!(reopened.instruction_timeline().steps, steps);
        reopened.undo(0).expect("undo complete append");
        assert!(reopened.instruction_timeline().steps.is_empty());
        assert!(!reopened.can_undo());
        assert!(reopened.can_redo());
        reopened.redo(1).expect("redo complete append");
        assert_eq!(reopened.instruction_timeline().steps, steps);
    }

    #[test]
    fn corrupt_inverse_index_is_rejected_without_a_panic_or_input_mutation() {
        let vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("add vertex");
        editor
            .execute(1, Command::RemoveVertex { id: vertex })
            .expect("remove vertex");
        let before = editor_document_parts_bytes(&editor).expect("snapshot input");
        let mut history = editor
            .export_history_v1(ProjectId::new())
            .expect("export history");
        let InverseV1::RestoreVertex { index, .. } = &mut history.undo_stack[1].inverse else {
            panic!("remove vertex must produce restore-vertex inverse");
        };
        *index = u32::MAX;

        assert!(matches!(
            restore(&editor, history),
            Err(EditorHistoryErrorV1::InvalidInverse)
        ));
        assert_eq!(
            editor_document_parts_bytes(&editor).expect("snapshot input after rejection"),
            before
        );
    }

    #[test]
    fn redo_inverse_must_match_the_canonical_inverse_generated_from_current() {
        let first = VertexId::new();
        let second = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: first,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("add first vertex");
        editor
            .execute(
                1,
                Command::AddVertex {
                    id: second,
                    position: Point2::new(3.0, 4.0),
                },
            )
            .expect("add second vertex");
        editor.undo(2).expect("create redo entry");
        let mut history = editor
            .export_history_v1(ProjectId::new())
            .expect("export history");
        history.redo_stack[0].inverse = InverseV1::Command {
            command: CommandV1::RemoveVertex { id: first },
        };

        assert!(matches!(
            restore(&editor, history),
            Err(EditorHistoryErrorV1::InverseMismatch)
        ));
    }

    #[test]
    fn replay_reauthenticates_signed_zero_bit_exactly() {
        let vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(-0.0, 2.0),
                },
            )
            .expect("add signed-zero vertex");
        let history = editor
            .export_history_v1(ProjectId::new())
            .expect("export signed-zero history");

        let reopened = restore(&editor, history.clone()).expect("restore signed-zero history");
        assert_eq!(
            reopened.pattern().vertices[0].position.x.to_bits(),
            (-0.0_f64).to_bits()
        );

        let mut mismatched = editor.clone();
        mismatched.pattern.vertices[0].position.x = 0.0;
        assert!(matches!(
            restore(&mismatched, history),
            Err(EditorHistoryErrorV1::CurrentDocumentMismatch)
        ));
    }

    #[test]
    fn layer_command_history_reopens_and_replays_exact_crud_and_assignments() {
        let project_id = ProjectId::new();
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(10.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        };
        let layer = LayerRecordV1 {
            id: LayerId::new(),
            name: "Details".to_owned(),
            content_kind: ori_domain::LayerContentKindV1::CreasePattern,
            visible: true,
            locked: false,
            opacity: 1.0,
        };
        let mut editor = EditorState::new(pattern.clone());
        editor
            .execute(
                0,
                Command::CreateLayer {
                    layer: layer.clone(),
                    target_index: 1,
                },
            )
            .expect("create layer");
        editor
            .execute(
                1,
                Command::AssignEdgeToLayer {
                    edge,
                    layer: layer.id,
                },
            )
            .expect("assign edge");
        editor
            .execute(
                2,
                Command::RenameLayer {
                    layer: layer.id,
                    name: "Renamed".to_owned(),
                },
            )
            .expect("rename layer");
        let expected = editor.project_layers().clone();
        let history = editor
            .export_history_v1(project_id)
            .expect("export layer history");

        let mut reopened = EditorState::with_document_parts_layers_and_history_v1(
            pattern,
            Paper::default(),
            InstructionTimeline::default(),
            GeometricConstraintDocumentV1::default(),
            expected.clone(),
            history,
        )
        .expect("reopen layer history");
        assert_eq!(reopened.project_layers(), &expected);
        for revision in 0..3 {
            reopened
                .undo(revision)
                .expect("undo reopened layer history");
        }
        assert_eq!(
            reopened.project_layers(),
            &ProjectLayerDocumentV1::default()
        );
        for revision in 3..6 {
            reopened
                .redo(revision)
                .expect("redo reopened layer history");
        }
        assert_eq!(reopened.project_layers(), &expected);
    }

    #[test]
    fn tampered_deleted_layer_inverse_is_rejected_before_input_mutation() {
        let project_id = ProjectId::new();
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(10.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Valley,
            }],
        };
        let layer = LayerRecordV1 {
            id: LayerId::new(),
            name: "Temporary".to_owned(),
            content_kind: ori_domain::LayerContentKindV1::CreasePattern,
            visible: true,
            locked: false,
            opacity: 1.0,
        };
        let mut editor = EditorState::new(pattern.clone());
        editor
            .execute(
                0,
                Command::CreateLayer {
                    layer: layer.clone(),
                    target_index: 1,
                },
            )
            .expect("create layer");
        editor
            .execute(
                1,
                Command::AssignEdgeToLayer {
                    edge,
                    layer: layer.id,
                },
            )
            .expect("assign edge");
        editor
            .execute(2, Command::DeleteLayer { layer: layer.id })
            .expect("delete assigned layer");
        let current_layers = editor.project_layers().clone();
        let mut history = editor
            .export_history_v1(project_id)
            .expect("export layer history");
        let InverseV1::RestoreDeletedLayer { index, .. } = &mut history.undo_stack[2].inverse
        else {
            panic!("delete command must store the deleted layer inverse");
        };
        *index = u32::MAX;
        let unchanged = history.clone();

        let result = EditorState::with_document_parts_layers_and_history_v1(
            pattern,
            Paper::default(),
            InstructionTimeline::default(),
            GeometricConstraintDocumentV1::default(),
            current_layers,
            history.clone(),
        );
        assert!(matches!(result, Err(EditorHistoryErrorV1::InvalidInverse)));
        assert_eq!(history, unchanged);
    }

    #[test]
    fn tampered_generated_edge_layer_inverse_is_rejected_before_input_mutation() {
        let project_id = ProjectId::new();
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let new_vertex = VertexId::new();
        let new_edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(10.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        };
        let layer = LayerRecordV1 {
            id: LayerId::new(),
            name: "Inherited".to_owned(),
            content_kind: ori_domain::LayerContentKindV1::CreasePattern,
            visible: true,
            locked: false,
            opacity: 1.0,
        };
        let mut layers = ProjectLayerDocumentV1::default();
        layers.layers.push(layer.clone());
        layers.edge_assignments.push(EdgeLayerAssignmentV1 {
            edge,
            layer: layer.id,
        });
        let mut editor = EditorState::with_document_parts_constraints_and_layers(
            pattern,
            Paper::default(),
            InstructionTimeline::default(),
            GeometricConstraintDocumentV1::default(),
            layers,
        );
        editor
            .execute(
                0,
                Command::SplitEdge {
                    edge,
                    new_vertex,
                    new_edge,
                    fraction: 0.5,
                },
            )
            .expect("split assigned edge");
        let mut history = editor
            .export_history_v1(project_id)
            .expect("export split history");
        let mut reopened = EditorState::with_document_parts_layers_and_history_v1(
            editor.pattern.clone(),
            editor.paper.clone(),
            editor.instruction_timeline.clone(),
            editor.geometric_constraints.clone(),
            editor.project_layers.clone(),
            history.clone(),
        )
        .expect("reopen inherited layer history");
        reopened.undo(0).expect("undo reopened assigned split");
        assert_eq!(
            reopened.project_layers().edge_assignments,
            vec![EdgeLayerAssignmentV1 {
                edge,
                layer: layer.id,
            }]
        );
        reopened.redo(1).expect("redo reopened assigned split");
        assert_eq!(reopened.project_layers().layer_for_edge(new_edge), layer.id);

        let InverseV1::RestoreEdgeSplit {
            new_edge_assignment,
            ..
        } = &mut history.undo_stack[0].inverse
        else {
            panic!("split command must store its generated-edge assignment");
        };
        assert!(new_edge_assignment.take().is_some());
        let unchanged = history.clone();

        let result = EditorState::with_document_parts_layers_and_history_v1(
            editor.pattern.clone(),
            editor.paper.clone(),
            editor.instruction_timeline.clone(),
            editor.geometric_constraints.clone(),
            editor.project_layers.clone(),
            history.clone(),
        );
        assert!(matches!(result, Err(EditorHistoryErrorV1::InvalidInverse)));
        assert_eq!(history, unchanged);
    }

    #[test]
    fn non_finite_wire_values_are_rejected_before_replay() {
        let mut history = history_with(128, vec![dummy_wire_entry()], Vec::new());
        history.undo_stack[0].forward = CommandV1::AddVertex {
            id: VertexId::new(),
            position: Point2::new(f64::NAN, 0.0),
        };
        let editor = EditorState::new(CreasePattern::empty());
        assert!(matches!(
            restore(&editor, history),
            Err(EditorHistoryErrorV1::NonFiniteNumber)
        ));
    }

    #[test]
    fn layer_presentation_wire_rejects_every_noncanonical_opacity_before_replay() {
        for (opacity, expected) in [
            (f64::NAN, EditorHistoryErrorV1::NonFiniteNumber),
            (f64::INFINITY, EditorHistoryErrorV1::NonFiniteNumber),
            (-0.0, EditorHistoryErrorV1::InvalidCommand),
            (-0.1, EditorHistoryErrorV1::InvalidCommand),
            (1.1, EditorHistoryErrorV1::InvalidCommand),
        ] {
            for corrupt_inverse in [false, true] {
                let mut entry = HistoryEntryV1 {
                    forward: CommandV1::UpdateLayerPresentation {
                        layer: DEFAULT_PROJECT_LAYER_ID,
                        visible: false,
                        locked: true,
                        opacity: 0.5,
                    },
                    inverse: InverseV1::Command {
                        command: CommandV1::UpdateLayerPresentation {
                            layer: DEFAULT_PROJECT_LAYER_ID,
                            visible: true,
                            locked: false,
                            opacity: 1.0,
                        },
                    },
                };
                if corrupt_inverse {
                    let InverseV1::Command {
                        command:
                            CommandV1::UpdateLayerPresentation {
                                opacity: inverse_opacity,
                                ..
                            },
                    } = &mut entry.inverse
                    else {
                        unreachable!("fixture inverse is a layer-presentation command");
                    };
                    *inverse_opacity = opacity;
                } else {
                    let CommandV1::UpdateLayerPresentation {
                        opacity: forward_opacity,
                        ..
                    } = &mut entry.forward
                    else {
                        unreachable!("fixture forward is a layer-presentation command");
                    };
                    *forward_opacity = opacity;
                }

                assert_eq!(
                    entry_from_wire(entry).expect_err("reject opacity"),
                    expected
                );
            }
        }
    }

    #[test]
    fn pose_transition_kind_is_rederived_from_endpoint_fingerprints() {
        let vertex = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(-0.0, 2.0),
                },
            )
            .expect("geometry edit");
        editor
            .execute(
                1,
                Command::MoveVertex {
                    id: vertex,
                    position: Point2::new(-0.0, 2.0),
                },
            )
            .expect("geometry no-op");
        editor
            .execute(2, Command::SetCuttingAllowed { allowed: true })
            .expect("non-geometry setting");

        let reopened = restore(
            &editor,
            editor
                .export_history_v1(ProjectId::new())
                .expect("export transition history"),
        )
        .expect("restore transition history");
        assert!(matches!(
            reopened.undo_stack[0].applied_pose,
            AppliedPoseHistoryTransition::Restore {
                before: None,
                after: None
            }
        ));
        assert!(matches!(
            reopened.undo_stack[1].applied_pose,
            AppliedPoseHistoryTransition::PreserveCurrent
        ));
        assert!(matches!(
            reopened.undo_stack[2].applied_pose,
            AppliedPoseHistoryTransition::PreserveCurrent
        ));
        assert!(reopened.current_applied_pose().is_none());
    }

    #[test]
    fn schema_nil_project_and_canonical_empty_contracts_are_fixed() {
        let editor = EditorState::new(CreasePattern::empty());
        let history = editor
            .export_history_v1(ProjectId::new())
            .expect("export empty history");
        assert!(history.is_default_empty());
        assert_eq!(
            serde_json::to_value(&history).expect("serialize empty history"),
            json!({
                "schema_version": 1,
                "project_id": history.project_id(),
                "history_entry_limit": 128,
                "undo_stack": [],
                "redo_stack": [],
            })
        );

        let nil_project: ProjectId =
            serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
                .expect("decode nil project ID");
        assert_eq!(
            editor.export_history_v1(nil_project),
            Err(EditorHistoryErrorV1::NilProjectId)
        );

        let mut unsupported = history;
        unsupported.schema_version += 1;
        assert_eq!(
            unsupported.validate_shape(),
            Err(EditorHistoryErrorV1::UnsupportedSchemaVersion)
        );
    }
}
