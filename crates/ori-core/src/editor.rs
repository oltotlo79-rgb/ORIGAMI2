use ori_domain::{CreasePattern, Point2, Vertex, VertexId};
use thiserror::Error;

pub type Revision = u64;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddVertex { id: VertexId, position: Point2 },
    MoveVertex { id: VertexId, position: Point2 },
    RemoveVertex { id: VertexId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandResult {
    pub revision: Revision,
    pub changed_vertices: Vec<VertexId>,
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
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    forward: Command,
    inverse: Command,
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pattern: CreasePattern,
    revision: Revision,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

impl EditorState {
    #[must_use]
    pub const fn new(pattern: CreasePattern) -> Self {
        Self {
            pattern,
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
        let changed_vertices = command.changed_vertices();
        self.undo_stack.push(HistoryEntry {
            forward: command,
            inverse,
        });
        self.redo_stack.clear();
        self.advance_revision();
        Ok(self.result(changed_vertices))
    }

    pub fn undo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let Some(entry) = self.undo_stack.pop() else {
            return Ok(self.result(Vec::new()));
        };
        let changed_vertices = entry.inverse.changed_vertices();
        self.apply(&entry.inverse)?;
        self.redo_stack.push(entry);
        self.advance_revision();
        Ok(self.result(changed_vertices))
    }

    pub fn redo(&mut self, expected_revision: Revision) -> Result<CommandResult, CommandError> {
        self.ensure_revision(expected_revision)?;
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(self.result(Vec::new()));
        };
        let changed_vertices = entry.forward.changed_vertices();
        self.apply(&entry.forward)?;
        self.undo_stack.push(entry);
        self.advance_revision();
        Ok(self.result(changed_vertices))
    }

    fn apply(&mut self, command: &Command) -> Result<Command, CommandError> {
        match *command {
            Command::AddVertex { id, position } => {
                if self.vertex_index(id).is_some() {
                    return Err(CommandError::VertexAlreadyExists(id));
                }
                self.pattern.vertices.push(Vertex { id, position });
                Ok(Command::RemoveVertex { id })
            }
            Command::MoveVertex { id, position } => {
                let index = self
                    .vertex_index(id)
                    .ok_or(CommandError::VertexNotFound(id))?;
                let previous = self.pattern.vertices[index].position;
                self.pattern.vertices[index].position = position;
                Ok(Command::MoveVertex {
                    id,
                    position: previous,
                })
            }
            Command::RemoveVertex { id } => {
                let index = self
                    .vertex_index(id)
                    .ok_or(CommandError::VertexNotFound(id))?;
                let vertex = self.pattern.vertices.remove(index);
                Ok(Command::AddVertex {
                    id: vertex.id,
                    position: vertex.position,
                })
            }
        }
    }

    fn vertex_index(&self, id: VertexId) -> Option<usize> {
        self.pattern
            .vertices
            .iter()
            .position(|vertex| vertex.id == id)
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

    fn result(&self, changed_vertices: Vec<VertexId>) -> CommandResult {
        CommandResult {
            revision: self.revision,
            changed_vertices,
        }
    }
}

impl Command {
    fn changed_vertices(&self) -> Vec<VertexId> {
        vec![match *self {
            Self::AddVertex { id, .. }
            | Self::MoveVertex { id, .. }
            | Self::RemoveVertex { id } => id,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
