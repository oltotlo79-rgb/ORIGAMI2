use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, RgbaColor, Vertex, VertexId,
};
use thiserror::Error;

pub type Revision = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddVertex {
        id: VertexId,
        position: Point2,
    },
    MoveVertex {
        id: VertexId,
        position: Point2,
    },
    RemoveVertex {
        id: VertexId,
    },
    AddEdge {
        id: EdgeId,
        start: VertexId,
        end: VertexId,
        kind: EdgeKind,
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
        cutting_allowed: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
    pub revision: Revision,
    pub changed_vertices: Vec<VertexId>,
    pub changed_edges: Vec<EdgeId>,
    pub settings_changed: bool,
}

#[derive(Debug, Error, PartialEq)]
pub enum CommandError {
    #[error("expected revision {expected}, but the current revision is {actual}")]
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    #[error("vertex {0:?} already exists")]
    VertexAlreadyExists(VertexId),
    #[error("vertex {0:?} was not found")]
    VertexNotFound(VertexId),
    #[error("edge {0:?} already exists")]
    EdgeAlreadyExists(EdgeId),
    #[error("edge {0:?} was not found")]
    EdgeNotFound(EdgeId),
    #[error("an edge cannot connect vertex {0:?} to itself")]
    DegenerateEdge(VertexId),
    #[error("vertex {vertex:?} is still used by edge {edge:?}")]
    VertexHasConnectedEdge { vertex: VertexId, edge: EdgeId },
    #[error("cut edges are disabled for this project")]
    CuttingDisabled,
    #[error("paper thickness must be finite")]
    PaperThicknessNotFinite,
    #[error("paper thickness must be zero or greater")]
    PaperThicknessNegative,
    #[error("cutting cannot be disabled while cut edge {edge:?} exists")]
    CutEdgesPreventDisabling { edge: EdgeId },
    #[error("boundary edge {0:?} must be changed through a sheet-boundary operation")]
    BoundaryEdgeRequiresSheetOperation(EdgeId),
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    forward: Command,
    inverse: Inverse,
}

#[derive(Debug, Clone)]
enum Inverse {
    Command(Command),
    RestoreVertex {
        index: usize,
        vertex: Vertex,
    },
    RestoreEdge {
        index: usize,
        edge: Edge,
    },
    RestorePaperProperties {
        thickness_mm: f64,
        front_color: RgbaColor,
        back_color: RgbaColor,
        cutting_allowed: bool,
    },
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pattern: CreasePattern,
    paper: Paper,
    revision: Revision,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

impl EditorState {
    #[must_use]
    pub fn new(pattern: CreasePattern) -> Self {
        Self::with_paper(pattern, Paper::default())
    }

    /// Restores an editor from a pattern and its persisted paper definition.
    ///
    /// The restored state starts at revision zero with empty undo and redo
    /// histories. Loading persisted paper data therefore cannot be undone as an
    /// editing operation.
    #[must_use]
    pub const fn with_paper(pattern: CreasePattern, paper: Paper) -> Self {
        Self {
            pattern,
            paper,
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    #[must_use]
    pub const fn pattern(&self) -> &CreasePattern {
        &self.pattern
    }

    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    #[must_use]
    pub const fn paper(&self) -> &Paper {
        &self.paper
    }

    #[must_use]
    pub const fn cutting_allowed(&self) -> bool {
        self.paper.cutting_allowed
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn execute(
        &mut self,
        expected_revision: Revision,
        command: Command,
    ) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let inverse = self.apply(&command)?;
        let result = command.changes();
        self.undo_stack.push(HistoryEntry {
            forward: command,
            inverse,
        });
        self.redo_stack.clear();
        self.advance_revision();
        Ok(self.result(result))
    }

    pub fn undo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let Some(entry) = self.undo_stack.pop() else {
            return Ok(self.result(Changes::default()));
        };
        let result = entry.inverse.changes();
        self.apply_inverse(&entry.inverse)?;
        self.redo_stack.push(entry);
        self.advance_revision();
        Ok(self.result(result))
    }

    pub fn redo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(self.result(Changes::default()));
        };
        let result = entry.forward.changes();
        self.apply(&entry.forward)?;
        self.undo_stack.push(entry);
        self.advance_revision();
        Ok(self.result(result))
    }

    fn apply(&mut self, command: &Command) -> Result<Inverse, CommandError> {
        match *command {
            Command::AddVertex { id, position } => {
                if self.vertex_index(id).is_some() {
                    return Err(CommandError::VertexAlreadyExists(id));
                }
                self.pattern.vertices.push(Vertex { id, position });
                Ok(Inverse::Command(Command::RemoveVertex { id }))
            }
            Command::MoveVertex { id, position } => {
                let index = self
                    .vertex_index(id)
                    .ok_or(CommandError::VertexNotFound(id))?;
                let previous = self.pattern.vertices[index].position;
                self.pattern.vertices[index].position = position;
                Ok(Inverse::Command(Command::MoveVertex {
                    id,
                    position: previous,
                }))
            }
            Command::RemoveVertex { id } => {
                if let Some(edge) = self
                    .pattern
                    .edges
                    .iter()
                    .find(|edge| edge.start == id || edge.end == id)
                {
                    return Err(CommandError::VertexHasConnectedEdge {
                        vertex: id,
                        edge: edge.id,
                    });
                }
                let index = self
                    .vertex_index(id)
                    .ok_or(CommandError::VertexNotFound(id))?;
                let vertex = self.pattern.vertices.remove(index);
                Ok(Inverse::RestoreVertex { index, vertex })
            }
            Command::AddEdge {
                id,
                start,
                end,
                kind,
            } => {
                if kind == EdgeKind::Boundary {
                    return Err(CommandError::BoundaryEdgeRequiresSheetOperation(id));
                }
                if kind == EdgeKind::Cut && !self.paper.cutting_allowed {
                    return Err(CommandError::CuttingDisabled);
                }
                if self.edge_index(id).is_some() {
                    return Err(CommandError::EdgeAlreadyExists(id));
                }
                if start == end {
                    return Err(CommandError::DegenerateEdge(start));
                }
                if self.vertex_index(start).is_none() {
                    return Err(CommandError::VertexNotFound(start));
                }
                if self.vertex_index(end).is_none() {
                    return Err(CommandError::VertexNotFound(end));
                }
                self.pattern.edges.push(Edge {
                    id,
                    start,
                    end,
                    kind,
                });
                Ok(Inverse::Command(Command::RemoveEdge { id }))
            }
            Command::RemoveEdge { id } => {
                let index = self.edge_index(id).ok_or(CommandError::EdgeNotFound(id))?;
                if self.pattern.edges[index].kind == EdgeKind::Boundary {
                    return Err(CommandError::BoundaryEdgeRequiresSheetOperation(id));
                }
                let edge = self.pattern.edges.remove(index);
                Ok(Inverse::RestoreEdge { index, edge })
            }
            Command::SetCuttingAllowed { allowed } => {
                self.ensure_cutting_can_be_set(allowed)?;
                let previous = self.paper.cutting_allowed;
                self.paper.cutting_allowed = allowed;
                Ok(Inverse::RestorePaperProperties {
                    thickness_mm: self.paper.thickness_mm,
                    front_color: self.paper.front.color,
                    back_color: self.paper.back.color,
                    cutting_allowed: previous,
                })
            }
            Command::UpdatePaperProperties {
                thickness_mm,
                front_color,
                back_color,
                cutting_allowed,
            } => {
                Self::validate_paper_thickness(thickness_mm)?;
                self.ensure_cutting_can_be_set(cutting_allowed)?;
                let inverse = Inverse::RestorePaperProperties {
                    thickness_mm: self.paper.thickness_mm,
                    front_color: self.paper.front.color,
                    back_color: self.paper.back.color,
                    cutting_allowed: self.paper.cutting_allowed,
                };
                self.paper.thickness_mm = thickness_mm;
                self.paper.front.color = front_color;
                self.paper.back.color = back_color;
                self.paper.cutting_allowed = cutting_allowed;
                Ok(inverse)
            }
        }
    }

    fn validate_paper_thickness(thickness_mm: f64) -> Result<(), CommandError> {
        if !thickness_mm.is_finite() {
            return Err(CommandError::PaperThicknessNotFinite);
        }
        if thickness_mm < 0.0 {
            return Err(CommandError::PaperThicknessNegative);
        }
        Ok(())
    }

    fn ensure_cutting_can_be_set(&self, allowed: bool) -> Result<(), CommandError> {
        if self.paper.cutting_allowed
            && !allowed
            && let Some(edge) = self
                .pattern
                .edges
                .iter()
                .find(|edge| edge.kind == EdgeKind::Cut)
        {
            return Err(CommandError::CutEdgesPreventDisabling { edge: edge.id });
        }
        Ok(())
    }

    fn apply_inverse(&mut self, inverse: &Inverse) -> Result<(), CommandError> {
        match inverse {
            Inverse::Command(command) => {
                self.apply(command)?;
            }
            Inverse::RestoreVertex { index, vertex } => {
                debug_assert!(self.vertex_index(vertex.id).is_none());
                debug_assert!(*index <= self.pattern.vertices.len());
                self.pattern.vertices.insert(*index, vertex.clone());
            }
            Inverse::RestoreEdge { index, edge } => {
                debug_assert!(self.edge_index(edge.id).is_none());
                debug_assert!(*index <= self.pattern.edges.len());
                self.pattern.edges.insert(*index, edge.clone());
            }
            Inverse::RestorePaperProperties {
                thickness_mm,
                front_color,
                back_color,
                cutting_allowed,
            } => {
                self.paper.thickness_mm = *thickness_mm;
                self.paper.front.color = *front_color;
                self.paper.back.color = *back_color;
                self.paper.cutting_allowed = *cutting_allowed;
            }
        }
        Ok(())
    }

    fn vertex_index(&self, id: VertexId) -> Option<usize> {
        self.pattern
            .vertices
            .iter()
            .position(|vertex| vertex.id == id)
    }

    fn edge_index(&self, id: EdgeId) -> Option<usize> {
        self.pattern.edges.iter().position(|edge| edge.id == id)
    }

    const fn ensure_revision(&self, expected: Revision) -> Result<(), CommandError> {
        if expected == self.revision {
            Ok(())
        } else {
            Err(CommandError::RevisionConflict {
                expected,
                actual: self.revision,
            })
        }
    }

    fn advance_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }

    fn result(&self, changes: Changes) -> CommandResult {
        CommandResult {
            revision: self.revision,
            changed_vertices: changes.vertices,
            changed_edges: changes.edges,
            settings_changed: changes.settings,
        }
    }
}

#[derive(Default)]
struct Changes {
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
    settings: bool,
}

impl Command {
    fn changes(&self) -> Changes {
        match *self {
            Self::AddVertex { id, .. }
            | Self::MoveVertex { id, .. }
            | Self::RemoveVertex { id } => Changes {
                vertices: vec![id],
                edges: Vec::new(),
                settings: false,
            },
            Self::AddEdge { id, start, end, .. } => Changes {
                vertices: vec![start, end],
                edges: vec![id],
                settings: false,
            },
            Self::RemoveEdge { id } => Changes {
                vertices: Vec::new(),
                edges: vec![id],
                settings: false,
            },
            Self::SetCuttingAllowed { .. } | Self::UpdatePaperProperties { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: true,
            },
        }
    }
}

impl Inverse {
    fn changes(&self) -> Changes {
        match self {
            Self::Command(command) => command.changes(),
            Self::RestoreVertex { vertex, .. } => Changes {
                vertices: vec![vertex.id],
                edges: Vec::new(),
                settings: false,
            },
            Self::RestoreEdge { edge, .. } => Changes {
                vertices: vec![edge.start, edge.end],
                edges: vec![edge.id],
                settings: false,
            },
            Self::RestorePaperProperties { .. } => Changes {
                vertices: Vec::new(),
                edges: Vec::new(),
                settings: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_only_constructor_uses_default_paper_without_history() {
        let editor = EditorState::new(CreasePattern::empty());

        assert_eq!(editor.paper(), &Paper::default());
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn add_move_undo_redo_preserves_vertex_id() {
        let id = VertexId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id,
                    position: Point2::new(1.0, 2.0),
                },
            )
            .expect("add vertex");
        editor
            .execute(
                1,
                Command::MoveVertex {
                    id,
                    position: Point2::new(5.0, 8.0),
                },
            )
            .expect("move vertex");
        editor.undo(2).expect("undo move");
        assert_eq!(editor.pattern().vertices[0].position, Point2::new(1.0, 2.0));
        editor.redo(3).expect("redo move");
        assert_eq!(editor.pattern().vertices[0].id, id);
        assert_eq!(editor.pattern().vertices[0].position, Point2::new(5.0, 8.0));
    }

    #[test]
    fn rejects_stale_revision_without_mutation() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let error = editor
            .execute(
                9,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect_err("stale command must fail");
        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 9,
                actual: 0
            }
        );
        assert!(editor.pattern().vertices.is_empty());
    }

    #[test]
    fn edge_is_undoable_and_keeps_its_id() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let mut editor = EditorState::new(CreasePattern::empty());
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
            )
            .expect("add start");
        editor
            .execute(
                1,
                Command::AddVertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            )
            .expect("add end");
        editor
            .execute(
                2,
                Command::AddEdge {
                    id: edge,
                    start,
                    end,
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("add edge");
        editor.undo(3).expect("undo edge");
        assert!(editor.pattern().edges.is_empty());
        editor.redo(4).expect("redo edge");
        assert_eq!(editor.pattern().edges[0].id, edge);
    }

    #[test]
    fn connected_vertex_cannot_be_removed() {
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
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Valley,
            }],
        };
        let mut editor = EditorState::new(pattern);
        let error = editor
            .execute(0, Command::RemoveVertex { id: start })
            .expect_err("connected vertex removal must fail");
        assert_eq!(
            error,
            CommandError::VertexHasConnectedEdge {
                vertex: start,
                edge
            }
        );
    }

    #[test]
    fn boundary_edge_cannot_be_added_by_a_generic_command() {
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
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(pattern);

        let error = editor
            .execute(
                0,
                Command::AddEdge {
                    id: edge,
                    start,
                    end,
                    kind: EdgeKind::Boundary,
                },
            )
            .expect_err("generic boundary creation must fail");

        assert_eq!(
            error,
            CommandError::BoundaryEdgeRequiresSheetOperation(edge)
        );
        assert!(editor.pattern().edges.is_empty());
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn boundary_edge_cannot_be_removed_by_a_generic_command() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let boundary = Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Boundary,
        };
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![boundary.clone()],
        };
        let mut editor = EditorState::new(pattern);

        let error = editor
            .execute(0, Command::RemoveEdge { id: edge })
            .expect_err("generic boundary removal must fail");

        assert_eq!(
            error,
            CommandError::BoundaryEdgeRequiresSheetOperation(edge)
        );
        assert_eq!(editor.pattern().edges, vec![boundary]);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn cut_edges_require_an_undoable_project_setting() {
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
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(pattern);
        let cut = Command::AddEdge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Cut,
        };
        assert_eq!(
            editor
                .execute(0, cut.clone())
                .expect_err("cut must be disabled"),
            CommandError::CuttingDisabled
        );
        editor
            .execute(0, Command::SetCuttingAllowed { allowed: true })
            .expect("enable cutting");
        editor.execute(1, cut).expect("add cut");
        assert_eq!(editor.pattern().edges.len(), 1);
        editor.undo(2).expect("undo cut");
        editor.undo(3).expect("undo setting");
        assert!(!editor.cutting_allowed());
    }

    #[test]
    fn paper_properties_are_one_undoable_command_and_preserve_textures() {
        let front_texture = ori_domain::AssetId::new();
        let back_texture = ori_domain::AssetId::new();
        let mut paper = Paper::default();
        paper.front.texture_asset = Some(front_texture);
        paper.back.texture_asset = Some(back_texture);
        let original = paper.clone();
        let mut editor = EditorState::with_paper(CreasePattern::empty(), paper);
        let front_color = RgbaColor::opaque(12, 34, 56);
        let back_color = RgbaColor::opaque(210, 190, 170);

        let result = editor
            .execute(
                0,
                Command::UpdatePaperProperties {
                    thickness_mm: 0.0,
                    front_color,
                    back_color,
                    cutting_allowed: true,
                },
            )
            .expect("update paper properties");

        assert_eq!(result.revision, 1);
        assert!(result.settings_changed);
        assert!(result.changed_vertices.is_empty());
        assert!(result.changed_edges.is_empty());
        assert_eq!(editor.paper().thickness_mm, 0.0);
        assert_eq!(editor.paper().front.color, front_color);
        assert_eq!(editor.paper().back.color, back_color);
        assert_eq!(editor.paper().front.texture_asset, Some(front_texture));
        assert_eq!(editor.paper().back.texture_asset, Some(back_texture));
        assert!(editor.paper().cutting_allowed);

        editor.undo(1).expect("undo paper properties");
        assert_eq!(editor.paper(), &original);
        editor.redo(2).expect("redo paper properties");
        assert_eq!(editor.paper().thickness_mm, 0.0);
        assert_eq!(editor.paper().front.color, front_color);
        assert_eq!(editor.paper().back.color, back_color);
        assert_eq!(editor.paper().front.texture_asset, Some(front_texture));
        assert_eq!(editor.paper().back.texture_asset, Some(back_texture));
        assert!(editor.paper().cutting_allowed);
    }

    #[test]
    fn invalid_paper_thickness_does_not_change_state_or_history() {
        for (invalid, expected) in [
            (f64::NAN, CommandError::PaperThicknessNotFinite),
            (f64::INFINITY, CommandError::PaperThicknessNotFinite),
            (f64::NEG_INFINITY, CommandError::PaperThicknessNotFinite),
            (-f64::MIN_POSITIVE, CommandError::PaperThicknessNegative),
        ] {
            let mut editor = EditorState::new(CreasePattern::empty());
            let original = editor.paper().clone();
            let error = editor
                .execute(
                    0,
                    Command::UpdatePaperProperties {
                        thickness_mm: invalid,
                        front_color: RgbaColor::opaque(1, 2, 3),
                        back_color: RgbaColor::opaque(4, 5, 6),
                        cutting_allowed: true,
                    },
                )
                .expect_err("invalid thickness must fail");

            assert_eq!(error, expected);
            assert_eq!(editor.paper(), &original);
            assert_eq!(editor.revision(), 0);
            assert!(!editor.can_undo());
            assert!(!editor.can_redo());
        }
    }

    #[test]
    fn existing_cut_edge_prevents_disabling_cutting_without_partial_changes() {
        let start = VertexId::new();
        let end = VertexId::new();
        let cut_edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: cut_edge,
                start,
                end,
                kind: EdgeKind::Cut,
            }],
        };
        let paper = Paper {
            cutting_allowed: true,
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(pattern.clone(), paper);
        let original = editor.paper().clone();

        let error = editor
            .execute(0, Command::SetCuttingAllowed { allowed: false })
            .expect_err("cut edge must prevent disabling");
        assert_eq!(
            error,
            CommandError::CutEdgesPreventDisabling { edge: cut_edge }
        );
        assert_eq!(editor.paper(), &original);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());

        let error = editor
            .execute(
                0,
                Command::UpdatePaperProperties {
                    thickness_mm: 0.5,
                    front_color: RgbaColor::opaque(1, 2, 3),
                    back_color: RgbaColor::opaque(4, 5, 6),
                    cutting_allowed: false,
                },
            )
            .expect_err("combined update must also reject disabling");
        assert_eq!(
            error,
            CommandError::CutEdgesPreventDisabling { edge: cut_edge }
        );
        assert_eq!(editor.paper(), &original);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn undo_restores_a_loaded_cutting_policy_without_revalidating_history() {
        let start = VertexId::new();
        let end = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Cut,
            }],
        };
        let mut editor = EditorState::with_paper(pattern, Paper::default());

        editor
            .execute(0, Command::SetCuttingAllowed { allowed: true })
            .expect("repair loaded cutting policy");
        editor.undo(1).expect("restore exact loaded state");

        assert!(!editor.paper().cutting_allowed);
        assert_eq!(editor.revision(), 2);
        assert!(editor.can_redo());
    }

    #[test]
    fn persisted_paper_restores_without_creating_history() {
        let paper = Paper {
            cutting_allowed: true,
            thickness_mm: 0.25,
            ..Paper::default()
        };
        let editor = EditorState::with_paper(CreasePattern::empty(), paper.clone());

        assert_eq!(editor.paper(), &paper);
        assert!(editor.cutting_allowed());
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn restored_cutting_setting_allows_cut_edges_at_revision_zero() {
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
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: Vec::new(),
        };
        let paper = Paper {
            cutting_allowed: true,
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::AddEdge {
                    id: edge,
                    start,
                    end,
                    kind: EdgeKind::Cut,
                },
            )
            .expect("add cut using restored setting");

        assert_eq!(editor.pattern().edges[0].id, edge);
        assert_eq!(editor.revision(), 1);
        assert!(editor.can_undo());
    }

    #[test]
    fn undo_remove_vertex_restores_the_original_vector_order() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let original = CreasePattern {
            vertices: vertices.to_vec(),
            edges: Vec::new(),
        };
        let mut editor = EditorState::new(original.clone());

        editor
            .execute(0, Command::RemoveVertex { id: vertices[1].id })
            .expect("remove middle vertex");
        editor.undo(1).expect("restore middle vertex");

        assert_eq!(editor.pattern(), &original);
    }

    #[test]
    fn undo_remove_edge_restores_the_original_vector_order() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edges = [
            Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start,
                end,
                kind: EdgeKind::Auxiliary,
            },
        ];
        let original = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: edges.to_vec(),
        };
        let mut editor = EditorState::new(original.clone());

        editor
            .execute(0, Command::RemoveEdge { id: edges[1].id })
            .expect("remove middle edge");
        editor.undo(1).expect("restore middle edge");

        assert_eq!(editor.pattern(), &original);
    }
}
