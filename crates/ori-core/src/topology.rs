use ori_domain::{CreasePattern, Paper, ProjectId};
use ori_topology::{FaceExtractionInput, FaceExtractionReport, analyze_faces};

use crate::{EditorState, Revision};

/// Owned, immutable input for topology work outside the editor-state lock.
///
/// Capturing paper and pattern together associates both with one revision.
/// A desktop caller can then release its project mutex before running the
/// potentially expensive face extraction.
#[derive(Debug, Clone, PartialEq)]
pub struct TopologyAnalysisInput {
    identity_namespace: ProjectId,
    revision: Revision,
    paper: Paper,
    pattern: CreasePattern,
}

impl TopologyAnalysisInput {
    /// Runs read-only face extraction against the captured revision.
    #[must_use]
    pub fn analyze(&self) -> EditorTopology {
        EditorTopology {
            revision: self.revision,
            report: analyze_faces(FaceExtractionInput {
                identity_namespace: self.identity_namespace,
                source_revision: self.revision,
                paper: &self.paper,
                pattern: &self.pattern,
            }),
        }
    }

    /// Returns the editor revision captured with this input.
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// Checks that a delayed result still describes the active editor input.
    ///
    /// Comparing paper and pattern as well as identity and revision prevents
    /// an ABA case where reopening the same project resets its revision.
    #[must_use]
    pub fn is_current_for(&self, identity_namespace: ProjectId, editor: &EditorState) -> bool {
        self.identity_namespace == identity_namespace
            && self.revision == editor.revision()
            && paper_bits_equal(&self.paper, editor.paper())
            && pattern_bits_equal(&self.pattern, editor.pattern())
    }
}

/// Read-only topology report tied to one editor revision.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorTopology {
    revision: Revision,
    report: FaceExtractionReport,
}

impl EditorTopology {
    /// Returns the editor revision used for analysis.
    #[must_use]
    pub const fn revision(&self) -> Revision {
        self.revision
    }

    /// Returns the diagnostic topology report.
    #[must_use]
    pub const fn report(&self) -> &FaceExtractionReport {
        &self.report
    }

    /// Consumes this result and returns the diagnostic report.
    #[must_use]
    pub fn into_report(self) -> FaceExtractionReport {
        self.report
    }

    /// Returns whether this result is a safe, complete topology snapshot for
    /// downstream consumers. A consumer still decides whether it supports the
    /// snapshot's hinge count and kinematic constraint class.
    #[must_use]
    pub fn is_simulation_ready(&self) -> bool {
        self.report.snapshot.is_some()
            && self
                .report
                .issues
                .iter()
                .all(|issue| matches!(issue.severity, ori_topology::TopologyIssueSeverity::Warning))
    }

    /// Returns a snapshot only when no issue blocks folding simulation.
    #[must_use]
    pub fn simulation_snapshot(&self) -> Option<&ori_topology::TopologySnapshot> {
        self.is_simulation_ready()
            .then_some(self.report.snapshot.as_ref())
            .flatten()
    }
}

fn paper_bits_equal(first: &Paper, second: &Paper) -> bool {
    first.boundary_vertices == second.boundary_vertices
        && first.thickness_mm.to_bits() == second.thickness_mm.to_bits()
        && first.cutting_allowed == second.cutting_allowed
        && first.front == second.front
        && first.back == second.back
}

fn pattern_bits_equal(first: &CreasePattern, second: &CreasePattern) -> bool {
    first.vertices.len() == second.vertices.len()
        && first
            .vertices
            .iter()
            .zip(&second.vertices)
            .all(|(first, second)| {
                first.id == second.id
                    && first.position.x.to_bits() == second.position.x.to_bits()
                    && first.position.y.to_bits() == second.position.y.to_bits()
            })
        && first.edges == second.edges
}

impl EditorState {
    /// Captures the current paper and crease pattern for read-only topology.
    #[must_use]
    pub fn topology_analysis_input(&self, identity_namespace: ProjectId) -> TopologyAnalysisInput {
        TopologyAnalysisInput {
            identity_namespace,
            revision: self.revision(),
            paper: self.paper().clone(),
            pattern: self.pattern().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};

    use super::*;
    use crate::{Command, create_rectangular_sheet};

    fn rectangular_editor() -> EditorState {
        let sheet = create_rectangular_sheet(8.0, 6.0, false).expect("rectangle fixture");
        let (pattern, paper) = sheet.into_parts();
        EditorState::with_paper(pattern, paper)
    }

    fn parallel_fold_editor() -> EditorState {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(6.0, 0.0),
            Point2::new(8.0, 0.0),
            Point2::new(8.0, 6.0),
            Point2::new(6.0, 6.0),
            Point2::new(2.0, 6.0),
            Point2::new(0.0, 6.0),
        ];
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend([
            Edge {
                id: EdgeId::new(),
                start: vertices[1].id,
                end: vertices[6].id,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: vertices[2].id,
                end: vertices[5].id,
                kind: EdgeKind::Valley,
            },
        ]);
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        EditorState::with_paper(CreasePattern { vertices, edges }, paper)
    }

    #[test]
    fn boundary_sheet_analysis_is_revision_bound_and_read_only() {
        let editor = rectangular_editor();
        let namespace = ProjectId::new();
        let pattern_before = editor.pattern().clone();
        let paper_before = editor.paper().clone();
        let input = editor.topology_analysis_input(namespace);

        let topology = input.analyze();

        assert_eq!(topology.revision(), 0);
        assert!(topology.is_simulation_ready());
        let snapshot = topology.report().snapshot.as_ref().expect("one face");
        assert_eq!(snapshot.source_revision, 0);
        assert_eq!(snapshot.faces.len(), 1);
        assert!(snapshot.hinge_adjacency.is_empty());
        assert_eq!(editor.pattern(), &pattern_before);
        assert_eq!(editor.paper(), &paper_before);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
        assert!(input.is_current_for(namespace, &editor));
    }

    #[test]
    fn one_fold_analysis_produces_two_faces_and_one_hinge() {
        let mut editor = rectangular_editor();
        let endpoints = [
            editor.paper().boundary_vertices[0],
            editor.paper().boundary_vertices[2],
        ];
        editor
            .execute(
                0,
                Command::AddEdge {
                    id: EdgeId::new(),
                    start: endpoints[0],
                    end: endpoints[1],
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("add one diagonal fold");

        let topology = editor.topology_analysis_input(ProjectId::new()).analyze();

        assert_eq!(topology.revision(), 1);
        assert!(topology.is_simulation_ready());
        let snapshot = topology.report().snapshot.as_ref().expect("two faces");
        assert_eq!(snapshot.source_revision, 1);
        assert_eq!(snapshot.faces.len(), 2);
        assert_eq!(snapshot.hinge_adjacency.len(), 1);
    }

    #[test]
    fn multiple_folds_produce_a_simulation_safe_cellular_snapshot() {
        let editor = parallel_fold_editor();
        let topology = editor.topology_analysis_input(ProjectId::new()).analyze();

        assert_eq!(topology.revision(), 0);
        assert!(topology.is_simulation_ready());
        assert!(topology.report().issues.is_empty());
        let snapshot = topology
            .simulation_snapshot()
            .expect("three cellular faces");
        assert_eq!(snapshot.faces.len(), 3);
        assert_eq!(snapshot.hinge_adjacency.len(), 2);
    }

    #[test]
    fn captured_input_detects_revision_identity_and_same_revision_content_changes() {
        let mut editor = rectangular_editor();
        let namespace = ProjectId::new();
        let input = editor.topology_analysis_input(namespace);
        assert!(input.is_current_for(namespace, &editor));
        assert!(!input.is_current_for(ProjectId::new(), &editor));

        let vertex = editor.pattern().vertices[0].id;
        editor
            .execute(
                0,
                Command::MoveVertex {
                    id: vertex,
                    position: ori_domain::Point2::new(-1.0, 0.0),
                },
            )
            .expect("move boundary vertex");
        assert!(!input.is_current_for(namespace, &editor));

        let replacement = rectangular_editor();
        assert_eq!(replacement.revision(), input.revision());
        assert!(!input.is_current_for(namespace, &replacement));
    }

    #[test]
    fn captured_input_remains_current_with_unchanged_non_finite_auxiliary_data() {
        let mut editor = rectangular_editor();
        editor
            .execute(
                0,
                Command::AddVertex {
                    id: ori_domain::VertexId::new(),
                    position: ori_domain::Point2::new(f64::NAN, f64::INFINITY),
                },
            )
            .expect("editor preserves an isolated draft vertex");
        let namespace = ProjectId::new();
        let input = editor.topology_analysis_input(namespace);

        assert!(input.is_current_for(namespace, &editor));
        assert!(input.analyze().is_simulation_ready());
    }
}
