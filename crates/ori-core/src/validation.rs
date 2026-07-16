use ori_geometry::validate_crease_pattern;

use crate::{CreasePatternValidation, EditorState, Revision, ValidationIssue};

/// A read-only validation snapshot tied to a specific editor revision.
///
/// Including the revision lets asynchronous UI consumers discard a report if
/// the project has changed before the report is displayed.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorValidation {
    revision: Revision,
    report: CreasePatternValidation,
}

impl EditorValidation {
    /// Returns the editor revision that was validated.
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns the complete crease-pattern validation report.
    #[must_use]
    pub const fn report(&self) -> &CreasePatternValidation {
        &self.report
    }

    /// Returns every issue detected in the crease pattern.
    #[must_use]
    pub fn issues(&self) -> &[ValidationIssue] {
        &self.report.issues
    }

    /// Returns `true` when the crease pattern has no detected issues.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.report.is_valid()
    }

    /// Consumes this snapshot and returns the complete validation report.
    #[must_use]
    pub fn into_report(self) -> CreasePatternValidation {
        self.report
    }
}

impl EditorState {
    /// Validates the current crease pattern without changing editor state.
    ///
    /// Invalid project geometry is represented by issues in the returned
    /// report, not by an operational error. This allows callers to inspect all
    /// defects in one pass. The report is associated with [`Self::revision`]
    /// through [`EditorValidation::revision`].
    #[must_use]
    pub fn validation(&self) -> EditorValidation {
        EditorValidation {
            revision: self.revision(),
            report: validate_crease_pattern(self.pattern()),
        }
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};

    use super::*;
    use crate::Command;

    fn crossing_pattern() -> CreasePattern {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let edges = vec![
            Edge {
                id: EdgeId::new(),
                start: vertices[0].id,
                end: vertices[1].id,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: vertices[2].id,
                end: vertices[3].id,
                kind: EdgeKind::Valley,
            },
        ];

        CreasePattern {
            vertices: vertices.into(),
            edges,
        }
    }

    #[test]
    fn validation_reports_geometry_issues_at_the_current_revision() {
        let editor = EditorState::new(crossing_pattern());

        let validation = editor.validation();

        assert_eq!(validation.revision(), 0);
        assert!(!validation.is_valid());
        assert!(matches!(
            validation.issues(),
            [ValidationIssue::UnsplitIntersection { .. }]
        ));
        assert_eq!(validation.report().issues.as_slice(), validation.issues());
    }

    #[test]
    fn validation_is_read_only_and_tracks_later_revisions() {
        let mut editor = EditorState::new(CreasePattern::empty());
        let first = editor.validation();
        assert!(first.is_valid());
        assert_eq!(first.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());

        editor
            .execute(
                0,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(1.0, 1.0),
                },
            )
            .expect("add a vertex");
        let second = editor.validation();

        assert!(second.is_valid());
        assert_eq!(second.revision(), 1);
        assert_eq!(editor.revision(), 1);
        assert!(editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn validation_report_can_be_taken_from_the_snapshot() {
        let editor = EditorState::new(CreasePattern::empty());

        let report = editor.validation().into_report();

        assert!(report.is_valid());
    }
}
