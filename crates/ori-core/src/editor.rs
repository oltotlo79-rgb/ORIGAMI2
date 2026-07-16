use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, RgbaColor, Vertex, VertexId,
};
use ori_geometry::{validate_crease_pattern, validate_paper};
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
    ResizeRectangularPaper {
        width_mm: f64,
        height_mm: f64,
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
    #[error("paper width must be finite")]
    PaperWidthNotFinite,
    #[error("paper width must be greater than zero")]
    PaperWidthNotPositive,
    #[error("paper height must be finite")]
    PaperHeightNotFinite,
    #[error("paper height must be greater than zero")]
    PaperHeightNotPositive,
    #[error("target paper area is too large to represent safely")]
    PaperResizeAreaNotRepresentable,
    #[error("paper boundary must contain exactly four vertices, found {actual}")]
    RectangularPaperBoundaryVertexCount { actual: usize },
    #[error("paper boundary references vertex {vertex:?} more than once")]
    RectangularPaperBoundaryDuplicateVertex { vertex: VertexId },
    #[error("paper boundary vertex {0:?} was not found")]
    RectangularPaperBoundaryVertexNotFound(VertexId),
    #[error("paper boundary vertex {vertex:?} has a non-finite position")]
    RectangularPaperBoundaryPositionNotFinite { vertex: VertexId },
    #[error("paper boundary does not have a finite positive rectangular area")]
    RectangularPaperBoundaryAreaNotRepresentable,
    #[error("paper boundary is not a rectangle")]
    PaperBoundaryNotRectangle,
    #[error("paper boundary is a rectangle but is not axis-aligned")]
    PaperBoundaryNotAxisAligned,
    #[error("paper boundary vertices must list adjacent corners in boundary order")]
    PaperBoundaryVerticesNotAdjacent,
    #[error("paper resize scale cannot be represented as a positive finite number")]
    PaperResizeScaleNotRepresentable,
    #[error("resizing the paper would produce a non-finite position for vertex {vertex:?}")]
    PaperResizeVertexPositionNotFinite { vertex: VertexId },
    #[error("requested paper dimensions cannot be represented at the current boundary origin")]
    PaperResizeBoundaryNotRepresentable,
    #[error("edge {0:?} is not a boundary edge")]
    EdgeIsNotBoundary(EdgeId),
    #[error("target boundary edge ID {edge:?} occurs more than once")]
    BoundarySplitTargetEdgeIdAmbiguous { edge: EdgeId },
    #[error("boundary edge {0:?} does not match a consecutive paper-boundary pair")]
    BoundaryEdgeNotInPaperBoundary(EdgeId),
    #[error("boundary edge {edge:?} matches more than one paper-boundary pair")]
    BoundaryEdgeMatchesMultiplePaperSegments { edge: EdgeId },
    #[error("boundary split fraction must be finite")]
    BoundarySplitFractionNotFinite,
    #[error("boundary split fraction must be strictly between zero and one")]
    BoundarySplitFractionOutOfRange,
    #[error("boundary edge {edge:?} endpoint {vertex:?} has a non-finite position")]
    BoundarySplitEndpointPositionNotFinite { edge: EdgeId, vertex: VertexId },
    #[error("boundary split position is not finite")]
    BoundarySplitPositionNotFinite,
    #[error("boundary split position must be distinct from both edge endpoints")]
    BoundarySplitPositionNotDistinct,
    #[error("boundary split position is already occupied by vertex {vertex:?}")]
    BoundarySplitPositionOccupied { vertex: VertexId },
    #[error("removing a boundary vertex requires at least four boundary entries, found {actual}")]
    BoundaryVertexRemovalNeedsFourVertices { actual: usize },
    #[error("vertex {0:?} is not in the paper boundary")]
    VertexNotInPaperBoundary(VertexId),
    #[error("vertex {vertex:?} occurs more than once in the paper boundary")]
    BoundaryVertexOccursMultipleTimes { vertex: VertexId },
    #[error("vertex ID {vertex:?} has more than one pattern vertex record")]
    BoundaryVertexRecordAmbiguous { vertex: VertexId },
    #[error("boundary vertex {vertex:?} has the same previous and next vertex {neighbor:?}")]
    BoundaryVertexNeighborsNotDistinct {
        vertex: VertexId,
        neighbor: VertexId,
    },
    #[error("vertex {vertex:?} has no unique preceding boundary edge")]
    BoundaryVertexPrecedingEdgeMissing { vertex: VertexId },
    #[error("vertex {vertex:?} has multiple preceding boundary edges")]
    BoundaryVertexPrecedingEdgeAmbiguous { vertex: VertexId },
    #[error("vertex {vertex:?} has no unique following boundary edge")]
    BoundaryVertexFollowingEdgeMissing { vertex: VertexId },
    #[error("vertex {vertex:?} has multiple following boundary edges")]
    BoundaryVertexFollowingEdgeAmbiguous { vertex: VertexId },
    #[error("vertex {vertex:?} must have two distinct adjacent boundary edge records")]
    BoundaryVertexAdjacentEdgesNotDistinct { vertex: VertexId },
    #[error("adjacent boundary edge ID {edge:?} for vertex {vertex:?} occurs more than once")]
    BoundaryVertexAdjacentEdgeIdAmbiguous { vertex: VertexId, edge: EdgeId },
    #[error("vertex {vertex:?} is connected to additional edge {edge:?}")]
    BoundaryVertexHasAdditionalEdge { vertex: VertexId, edge: EdgeId },
    #[error("edge {edge:?} already connects the neighbors of boundary vertex {vertex:?}")]
    BoundaryVertexNeighborEdgeAlreadyExists { vertex: VertexId, edge: EdgeId },
    #[error("removing the boundary vertex would invalidate a currently valid paper")]
    BoundaryVertexRemovalWouldInvalidatePaper,
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
    RestoreVertexPositions {
        vertices: Vec<(VertexId, Point2)>,
    },
    RestoreBoundarySplit {
        boundary_vertices: Vec<VertexId>,
        original_edge_index: usize,
        original_edge: Edge,
        new_vertex_index: usize,
        new_vertex: Vertex,
        new_edge_index: usize,
        new_edge: Edge,
    },
    RestoreBoundaryVertexRemoval {
        boundary_index: usize,
        vertex_index: usize,
        vertex: Vertex,
        kept_edge_index: usize,
        kept_edge: Edge,
        removed_edge_index: usize,
        removed_edge: Edge,
        previous_vertex: VertexId,
        next_vertex: VertexId,
    },
}

#[derive(Debug, Clone, Copy)]
struct RectangularBoundary {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

fn undirected_endpoints_match(
    first_start: VertexId,
    first_end: VertexId,
    second_start: VertexId,
    second_end: VertexId,
) -> bool {
    (first_start == second_start && first_end == second_end)
        || (first_start == second_end && first_end == second_start)
}

fn stable_convex_combination(start: f64, end: f64, fraction: f64) -> f64 {
    if start.is_sign_negative() == end.is_sign_negative() {
        start + (end - start) * fraction
    } else {
        start * (1.0 - fraction) + end * fraction
    }
}

fn apply_boundary_vertex_removal(
    pattern: &mut CreasePattern,
    paper: &mut Paper,
    boundary_index: usize,
    vertex_index: usize,
    kept_edge_index: usize,
    removed_edge_index: usize,
    merged_edge: &Edge,
) {
    paper.boundary_vertices.remove(boundary_index);
    pattern.vertices.remove(vertex_index);
    pattern.edges[kept_edge_index] = merged_edge.clone();
    pattern.edges.remove(removed_edge_index);
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
        let result = command.changes(&self.pattern, &self.paper);
        let inverse = self.apply(&command)?;
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
        let result = entry.inverse.changes(&self.pattern, &self.paper);
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
        let result = entry.forward.changes(&self.pattern, &self.paper);
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
            Command::ResizeRectangularPaper {
                width_mm,
                height_mm,
            } => self.resize_rectangular_paper(width_mm, height_mm),
            Command::SplitBoundaryEdge {
                edge,
                new_vertex,
                new_edge,
                fraction,
            } => self.split_boundary_edge(edge, new_vertex, new_edge, fraction),
            Command::RemoveBoundaryVertex { vertex } => self.remove_boundary_vertex(vertex),
        }
    }

    fn remove_boundary_vertex(&mut self, vertex_id: VertexId) -> Result<Inverse, CommandError> {
        let boundary_len = self.paper.boundary_vertices.len();
        if boundary_len < 4 {
            return Err(CommandError::BoundaryVertexRemovalNeedsFourVertices {
                actual: boundary_len,
            });
        }

        let mut boundary_occurrences = self
            .paper
            .boundary_vertices
            .iter()
            .enumerate()
            .filter(|(_, candidate)| **candidate == vertex_id);
        let Some((boundary_index, _)) = boundary_occurrences.next() else {
            return Err(CommandError::VertexNotInPaperBoundary(vertex_id));
        };
        if boundary_occurrences.next().is_some() {
            return Err(CommandError::BoundaryVertexOccursMultipleTimes { vertex: vertex_id });
        }

        let mut vertex_records = self
            .pattern
            .vertices
            .iter()
            .enumerate()
            .filter(|(_, vertex)| vertex.id == vertex_id);
        let Some((vertex_index, vertex)) = vertex_records
            .next()
            .map(|(index, vertex)| (index, vertex.clone()))
        else {
            return Err(CommandError::VertexNotFound(vertex_id));
        };
        if vertex_records.next().is_some() {
            return Err(CommandError::BoundaryVertexRecordAmbiguous { vertex: vertex_id });
        }

        let previous_vertex =
            self.paper.boundary_vertices[(boundary_index + boundary_len - 1) % boundary_len];
        let next_vertex = self.paper.boundary_vertices[(boundary_index + 1) % boundary_len];
        if previous_vertex == next_vertex {
            return Err(CommandError::BoundaryVertexNeighborsNotDistinct {
                vertex: vertex_id,
                neighbor: previous_vertex,
            });
        }
        let preceding_edges = self.matching_boundary_edge_indices(previous_vertex, vertex_id);
        let kept_edge_index = match preceding_edges.as_slice() {
            [] => {
                return Err(CommandError::BoundaryVertexPrecedingEdgeMissing { vertex: vertex_id });
            }
            [index] => *index,
            _ => {
                return Err(CommandError::BoundaryVertexPrecedingEdgeAmbiguous {
                    vertex: vertex_id,
                });
            }
        };
        let following_edges = self.matching_boundary_edge_indices(vertex_id, next_vertex);
        let removed_edge_index = match following_edges.as_slice() {
            [] => {
                return Err(CommandError::BoundaryVertexFollowingEdgeMissing { vertex: vertex_id });
            }
            [index] => *index,
            _ => {
                return Err(CommandError::BoundaryVertexFollowingEdgeAmbiguous {
                    vertex: vertex_id,
                });
            }
        };
        let kept_edge = self.pattern.edges[kept_edge_index].clone();
        let removed_edge = self.pattern.edges[removed_edge_index].clone();
        if kept_edge_index == removed_edge_index || kept_edge.id == removed_edge.id {
            return Err(CommandError::BoundaryVertexAdjacentEdgesNotDistinct { vertex: vertex_id });
        }
        for edge in [&kept_edge, &removed_edge] {
            if self
                .pattern
                .edges
                .iter()
                .filter(|candidate| candidate.id == edge.id)
                .count()
                != 1
            {
                return Err(CommandError::BoundaryVertexAdjacentEdgeIdAmbiguous {
                    vertex: vertex_id,
                    edge: edge.id,
                });
            }
        }

        if let Some(edge) = self
            .pattern
            .edges
            .iter()
            .enumerate()
            .find(|(index, edge)| {
                *index != kept_edge_index
                    && *index != removed_edge_index
                    && (edge.start == vertex_id || edge.end == vertex_id)
            })
            .map(|(_, edge)| edge)
        {
            return Err(CommandError::BoundaryVertexHasAdditionalEdge {
                vertex: vertex_id,
                edge: edge.id,
            });
        }
        if let Some(edge) = self.pattern.edges.iter().find(|edge| {
            undirected_endpoints_match(edge.start, edge.end, previous_vertex, next_vertex)
        }) {
            return Err(CommandError::BoundaryVertexNeighborEdgeAlreadyExists {
                vertex: vertex_id,
                edge: edge.id,
            });
        }

        let mut merged_edge = kept_edge.clone();
        if merged_edge.start == vertex_id {
            merged_edge.start = next_vertex;
        } else {
            debug_assert_eq!(merged_edge.end, vertex_id);
            merged_edge.end = next_vertex;
        }

        let current_crease_is_valid = validate_crease_pattern(&self.pattern).is_valid();
        let current_paper_is_valid = validate_paper(&self.paper, &self.pattern).is_valid();
        if current_crease_is_valid && current_paper_is_valid {
            let mut candidate_pattern = self.pattern.clone();
            let mut candidate_paper = self.paper.clone();
            apply_boundary_vertex_removal(
                &mut candidate_pattern,
                &mut candidate_paper,
                boundary_index,
                vertex_index,
                kept_edge_index,
                removed_edge_index,
                &merged_edge,
            );
            if !validate_crease_pattern(&candidate_pattern).is_valid()
                || !validate_paper(&candidate_paper, &candidate_pattern).is_valid()
            {
                return Err(CommandError::BoundaryVertexRemovalWouldInvalidatePaper);
            }
        }

        apply_boundary_vertex_removal(
            &mut self.pattern,
            &mut self.paper,
            boundary_index,
            vertex_index,
            kept_edge_index,
            removed_edge_index,
            &merged_edge,
        );

        Ok(Inverse::RestoreBoundaryVertexRemoval {
            boundary_index,
            vertex_index,
            vertex,
            kept_edge_index,
            kept_edge,
            removed_edge_index,
            removed_edge,
            previous_vertex,
            next_vertex,
        })
    }

    fn matching_boundary_edge_indices(&self, start: VertexId, end: VertexId) -> Vec<usize> {
        self.pattern
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| {
                edge.kind == EdgeKind::Boundary
                    && undirected_endpoints_match(edge.start, edge.end, start, end)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn split_boundary_edge(
        &mut self,
        edge_id: EdgeId,
        new_vertex_id: VertexId,
        new_edge_id: EdgeId,
        fraction: f64,
    ) -> Result<Inverse, CommandError> {
        if !fraction.is_finite() {
            return Err(CommandError::BoundarySplitFractionNotFinite);
        }
        if fraction <= 0.0 || fraction >= 1.0 {
            return Err(CommandError::BoundarySplitFractionOutOfRange);
        }
        if self.vertex_index(new_vertex_id).is_some()
            || self.paper.boundary_vertices.contains(&new_vertex_id)
            || self
                .pattern
                .edges
                .iter()
                .any(|edge| edge.start == new_vertex_id || edge.end == new_vertex_id)
        {
            return Err(CommandError::VertexAlreadyExists(new_vertex_id));
        }
        if self.edge_index(new_edge_id).is_some() {
            return Err(CommandError::EdgeAlreadyExists(new_edge_id));
        }

        let mut target_edges = self
            .pattern
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.id == edge_id);
        let Some((original_edge_index, original_edge)) = target_edges
            .next()
            .map(|(index, edge)| (index, edge.clone()))
        else {
            return Err(CommandError::EdgeNotFound(edge_id));
        };
        if target_edges.next().is_some() {
            return Err(CommandError::BoundarySplitTargetEdgeIdAmbiguous { edge: edge_id });
        }
        if original_edge.kind != EdgeKind::Boundary {
            return Err(CommandError::EdgeIsNotBoundary(edge_id));
        }

        let mut matching_boundary_indices = Vec::new();
        if !self.paper.boundary_vertices.is_empty() {
            for (index, start) in self.paper.boundary_vertices.iter().copied().enumerate() {
                let end =
                    self.paper.boundary_vertices[(index + 1) % self.paper.boundary_vertices.len()];
                if undirected_endpoints_match(original_edge.start, original_edge.end, start, end) {
                    matching_boundary_indices.push(index);
                }
            }
        }
        let boundary_index = match matching_boundary_indices.as_slice() {
            [] => return Err(CommandError::BoundaryEdgeNotInPaperBoundary(edge_id)),
            [index] => *index,
            _ => {
                return Err(CommandError::BoundaryEdgeMatchesMultiplePaperSegments {
                    edge: edge_id,
                });
            }
        };

        let start_index = self
            .vertex_index(original_edge.start)
            .ok_or(CommandError::VertexNotFound(original_edge.start))?;
        let end_index = self
            .vertex_index(original_edge.end)
            .ok_or(CommandError::VertexNotFound(original_edge.end))?;
        let start_position = self.pattern.vertices[start_index].position;
        let end_position = self.pattern.vertices[end_index].position;
        if !start_position.x.is_finite() || !start_position.y.is_finite() {
            return Err(CommandError::BoundarySplitEndpointPositionNotFinite {
                edge: edge_id,
                vertex: original_edge.start,
            });
        }
        if !end_position.x.is_finite() || !end_position.y.is_finite() {
            return Err(CommandError::BoundarySplitEndpointPositionNotFinite {
                edge: edge_id,
                vertex: original_edge.end,
            });
        }
        let position = Point2::new(
            stable_convex_combination(start_position.x, end_position.x, fraction),
            stable_convex_combination(start_position.y, end_position.y, fraction),
        );
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(CommandError::BoundarySplitPositionNotFinite);
        }
        if position == start_position || position == end_position {
            return Err(CommandError::BoundarySplitPositionNotDistinct);
        }
        if let Some(vertex) = self
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.position == position)
        {
            return Err(CommandError::BoundarySplitPositionOccupied { vertex: vertex.id });
        }

        let boundary_vertices = self.paper.boundary_vertices.clone();
        let new_vertex_index = self.pattern.vertices.len();
        let new_edge_index = original_edge_index + 1;
        let new_vertex = Vertex {
            id: new_vertex_id,
            position,
        };
        let new_edge = Edge {
            id: new_edge_id,
            start: new_vertex_id,
            end: original_edge.end,
            kind: EdgeKind::Boundary,
        };

        self.paper
            .boundary_vertices
            .insert(boundary_index + 1, new_vertex_id);
        self.pattern.vertices.push(new_vertex.clone());
        self.pattern.edges[original_edge_index].end = new_vertex_id;
        self.pattern.edges.insert(new_edge_index, new_edge.clone());

        Ok(Inverse::RestoreBoundarySplit {
            boundary_vertices,
            original_edge_index,
            original_edge,
            new_vertex_index,
            new_vertex,
            new_edge_index,
            new_edge,
        })
    }

    fn resize_rectangular_paper(
        &mut self,
        width_mm: f64,
        height_mm: f64,
    ) -> Result<Inverse, CommandError> {
        Self::validate_resize_dimensions(width_mm, height_mm)?;
        let boundary = self.rectangular_boundary()?;
        let same_width = width_mm == boundary.max_x - boundary.min_x;
        let same_height = height_mm == boundary.max_y - boundary.min_y;
        let target_max_x = if same_width {
            boundary.max_x
        } else {
            boundary.min_x + width_mm
        };
        let target_max_y = if same_height {
            boundary.max_y
        } else {
            boundary.min_y + height_mm
        };
        if !target_max_x.is_finite()
            || !target_max_y.is_finite()
            || target_max_x <= boundary.min_x
            || target_max_y <= boundary.min_y
        {
            return Err(CommandError::PaperResizeBoundaryNotRepresentable);
        }

        let current_width = boundary.max_x - boundary.min_x;
        let current_height = boundary.max_y - boundary.min_y;
        let scale_x = width_mm / current_width;
        let scale_y = height_mm / current_height;
        if !scale_x.is_finite() || scale_x <= 0.0 || !scale_y.is_finite() || scale_y <= 0.0 {
            return Err(CommandError::PaperResizeScaleNotRepresentable);
        }

        let previous_positions = self
            .pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<Vec<_>>();
        let mut resized_positions = Vec::with_capacity(self.pattern.vertices.len());
        for vertex in &self.pattern.vertices {
            if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
                return Err(CommandError::PaperResizeVertexPositionNotFinite { vertex: vertex.id });
            }
            let x = if same_width {
                vertex.position.x
            } else {
                boundary.min_x + (vertex.position.x - boundary.min_x) * scale_x
            };
            let y = if same_height {
                vertex.position.y
            } else {
                boundary.min_y + (vertex.position.y - boundary.min_y) * scale_y
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(CommandError::PaperResizeVertexPositionNotFinite { vertex: vertex.id });
            }
            resized_positions.push(Point2::new(x, y));
        }

        // Set the four corners explicitly so floating-point multiplication can
        // never leave a boundary corner just short of its requested target.
        for boundary_id in &self.paper.boundary_vertices {
            let index = self.vertex_index(*boundary_id).ok_or(
                CommandError::RectangularPaperBoundaryVertexNotFound(*boundary_id),
            )?;
            let original = self.pattern.vertices[index].position;
            resized_positions[index] = Point2::new(
                if original.x == boundary.min_x {
                    boundary.min_x
                } else {
                    target_max_x
                },
                if original.y == boundary.min_y {
                    boundary.min_y
                } else {
                    target_max_y
                },
            );
        }

        for (vertex, position) in self.pattern.vertices.iter_mut().zip(resized_positions) {
            vertex.position = position;
        }
        Ok(Inverse::RestoreVertexPositions {
            vertices: previous_positions,
        })
    }

    fn validate_resize_dimensions(width_mm: f64, height_mm: f64) -> Result<(), CommandError> {
        if !width_mm.is_finite() {
            return Err(CommandError::PaperWidthNotFinite);
        }
        if width_mm <= 0.0 {
            return Err(CommandError::PaperWidthNotPositive);
        }
        if !height_mm.is_finite() {
            return Err(CommandError::PaperHeightNotFinite);
        }
        if height_mm <= 0.0 {
            return Err(CommandError::PaperHeightNotPositive);
        }
        let doubled_area = width_mm * height_mm * 2.0;
        if !doubled_area.is_finite() || doubled_area <= 0.0 {
            return Err(CommandError::PaperResizeAreaNotRepresentable);
        }
        Ok(())
    }

    fn rectangular_boundary(&self) -> Result<RectangularBoundary, CommandError> {
        if self.paper.boundary_vertices.len() != 4 {
            return Err(CommandError::RectangularPaperBoundaryVertexCount {
                actual: self.paper.boundary_vertices.len(),
            });
        }

        for (index, vertex) in self.paper.boundary_vertices.iter().enumerate() {
            if self.paper.boundary_vertices[..index].contains(vertex) {
                return Err(CommandError::RectangularPaperBoundaryDuplicateVertex {
                    vertex: *vertex,
                });
            }
        }

        let mut points = [Point2::new(0.0, 0.0); 4];
        for (index, vertex_id) in self.paper.boundary_vertices.iter().enumerate() {
            let vertex_index = self.vertex_index(*vertex_id).ok_or(
                CommandError::RectangularPaperBoundaryVertexNotFound(*vertex_id),
            )?;
            let position = self.pattern.vertices[vertex_index].position;
            if !position.x.is_finite() || !position.y.is_finite() {
                return Err(CommandError::RectangularPaperBoundaryPositionNotFinite {
                    vertex: *vertex_id,
                });
            }
            points[index] = position;
        }

        let min_x = points
            .iter()
            .map(|point| point.x)
            .fold(f64::INFINITY, f64::min);
        let min_y = points
            .iter()
            .map(|point| point.y)
            .fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|point| point.x)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = points
            .iter()
            .map(|point| point.y)
            .fold(f64::NEG_INFINITY, f64::max);
        let width = max_x - min_x;
        let height = max_y - min_y;
        let doubled_area = width * height * 2.0;
        if !width.is_finite()
            || width <= 0.0
            || !height.is_finite()
            || height <= 0.0
            || !doubled_area.is_finite()
            || doubled_area <= 0.0
        {
            return Err(CommandError::RectangularPaperBoundaryAreaNotRepresentable);
        }

        let mut seen_corners = [false; 4];
        let mut corner_indices = [0usize; 4];
        let mut corners_match = true;
        for (index, point) in points.iter().enumerate() {
            let corner = match (
                point.x == min_x,
                point.x == max_x,
                point.y == min_y,
                point.y == max_y,
            ) {
                (true, false, true, false) => Some(0),
                (false, true, true, false) => Some(1),
                (false, true, false, true) => Some(2),
                (true, false, false, true) => Some(3),
                _ => None,
            };
            let Some(corner) = corner else {
                corners_match = false;
                break;
            };
            if seen_corners[corner] {
                corners_match = false;
                break;
            }
            seen_corners[corner] = true;
            corner_indices[index] = corner;
        }

        if !corners_match {
            return if Self::ordered_points_form_rectangle(points) {
                Err(CommandError::PaperBoundaryNotAxisAligned)
            } else {
                Err(CommandError::PaperBoundaryNotRectangle)
            };
        }

        let adjacent_pairs = [
            (corner_indices[0], corner_indices[1]),
            (corner_indices[1], corner_indices[2]),
            (corner_indices[2], corner_indices[3]),
            (corner_indices[3], corner_indices[0]),
        ];
        if adjacent_pairs
            .into_iter()
            .any(|(current, next)| current.abs_diff(next) == 2)
        {
            return Err(CommandError::PaperBoundaryVerticesNotAdjacent);
        }

        Ok(RectangularBoundary {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    fn ordered_points_form_rectangle(points: [Point2; 4]) -> bool {
        let edges = [
            (points[1].x - points[0].x, points[1].y - points[0].y),
            (points[2].x - points[1].x, points[2].y - points[1].y),
            (points[3].x - points[2].x, points[3].y - points[2].y),
            (points[0].x - points[3].x, points[0].y - points[3].y),
        ];
        if edges
            .iter()
            .any(|(x, y)| !x.is_finite() || !y.is_finite() || (*x == 0.0 && *y == 0.0))
        {
            return false;
        }
        let dot = edges[0].0 * edges[1].0 + edges[0].1 * edges[1].1;
        dot.is_finite()
            && dot == 0.0
            && edges[0].0 == -edges[2].0
            && edges[0].1 == -edges[2].1
            && edges[1].0 == -edges[3].0
            && edges[1].1 == -edges[3].1
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
            Inverse::RestoreVertexPositions { vertices } => {
                debug_assert_eq!(vertices.len(), self.pattern.vertices.len());
                for (vertex, (expected_id, position)) in
                    self.pattern.vertices.iter_mut().zip(vertices)
                {
                    debug_assert_eq!(vertex.id, *expected_id);
                    vertex.position = *position;
                }
            }
            Inverse::RestoreBoundarySplit {
                boundary_vertices,
                original_edge_index,
                original_edge,
                new_vertex_index,
                new_vertex,
                new_edge_index,
                new_edge,
            } => {
                debug_assert_eq!(
                    self.pattern.edges.get(*new_edge_index).map(|edge| edge.id),
                    Some(new_edge.id)
                );
                self.pattern.edges.remove(*new_edge_index);
                debug_assert_eq!(
                    self.pattern
                        .edges
                        .get(*original_edge_index)
                        .map(|edge| edge.id),
                    Some(original_edge.id)
                );
                self.pattern.edges[*original_edge_index] = original_edge.clone();
                debug_assert_eq!(
                    self.pattern
                        .vertices
                        .get(*new_vertex_index)
                        .map(|vertex| vertex.id),
                    Some(new_vertex.id)
                );
                self.pattern.vertices.remove(*new_vertex_index);
                self.paper.boundary_vertices = boundary_vertices.clone();
            }
            Inverse::RestoreBoundaryVertexRemoval {
                boundary_index,
                vertex_index,
                vertex,
                kept_edge_index,
                kept_edge,
                removed_edge_index,
                removed_edge,
                ..
            } => {
                let current_kept_index = if removed_edge_index < kept_edge_index {
                    *kept_edge_index - 1
                } else {
                    *kept_edge_index
                };
                debug_assert_eq!(
                    self.pattern
                        .edges
                        .get(current_kept_index)
                        .map(|edge| edge.id),
                    Some(kept_edge.id)
                );
                self.pattern
                    .edges
                    .insert(*removed_edge_index, removed_edge.clone());
                self.pattern.edges[*kept_edge_index] = kept_edge.clone();
                self.pattern.vertices.insert(*vertex_index, vertex.clone());
                self.paper
                    .boundary_vertices
                    .insert(*boundary_index, vertex.id);
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
    fn changes(&self, pattern: &CreasePattern, paper: &Paper) -> Changes {
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
            Self::ResizeRectangularPaper { .. } => Changes {
                vertices: pattern.vertices.iter().map(|vertex| vertex.id).collect(),
                edges: Vec::new(),
                settings: false,
            },
            Self::SplitBoundaryEdge {
                edge,
                new_vertex,
                new_edge,
                ..
            } => {
                let mut vertices = vec![new_vertex];
                if let Some(original_edge) =
                    pattern.edges.iter().find(|candidate| candidate.id == edge)
                {
                    vertices.push(original_edge.start);
                    vertices.push(original_edge.end);
                }
                Changes {
                    vertices,
                    edges: vec![edge, new_edge],
                    settings: true,
                }
            }
            Self::RemoveBoundaryVertex { vertex } => {
                let mut vertices = vec![vertex];
                let mut edges = Vec::new();
                if let Some(boundary_index) = paper
                    .boundary_vertices
                    .iter()
                    .position(|candidate| *candidate == vertex)
                {
                    let boundary_len = paper.boundary_vertices.len();
                    let previous =
                        paper.boundary_vertices[(boundary_index + boundary_len - 1) % boundary_len];
                    let next = paper.boundary_vertices[(boundary_index + 1) % boundary_len];
                    vertices.push(previous);
                    vertices.push(next);
                    if let Some(edge) = pattern.edges.iter().find(|edge| {
                        edge.kind == EdgeKind::Boundary
                            && undirected_endpoints_match(edge.start, edge.end, previous, vertex)
                    }) {
                        edges.push(edge.id);
                    }
                    if let Some(edge) = pattern.edges.iter().find(|edge| {
                        edge.kind == EdgeKind::Boundary
                            && undirected_endpoints_match(edge.start, edge.end, vertex, next)
                    }) {
                        edges.push(edge.id);
                    }
                }
                Changes {
                    vertices,
                    edges,
                    settings: true,
                }
            }
        }
    }
}

impl Inverse {
    fn changes(&self, pattern: &CreasePattern, paper: &Paper) -> Changes {
        match self {
            Self::Command(command) => command.changes(pattern, paper),
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
            Self::RestoreVertexPositions { vertices } => Changes {
                vertices: vertices.iter().map(|(id, _)| *id).collect(),
                edges: Vec::new(),
                settings: false,
            },
            Self::RestoreBoundarySplit {
                original_edge,
                new_vertex,
                new_edge,
                ..
            } => Changes {
                vertices: vec![new_vertex.id, original_edge.start, original_edge.end],
                edges: vec![original_edge.id, new_edge.id],
                settings: true,
            },
            Self::RestoreBoundaryVertexRemoval {
                vertex,
                kept_edge,
                removed_edge,
                previous_vertex,
                next_vertex,
                ..
            } => Changes {
                vertices: vec![vertex.id, *previous_vertex, *next_vertex],
                edges: vec![kept_edge.id, removed_edge.id],
                settings: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangular_editor() -> (EditorState, CreasePattern, Paper) {
        let bottom_left = VertexId::new();
        let bottom_right = VertexId::new();
        let top_right = VertexId::new();
        let top_left = VertexId::new();
        let internal = VertexId::new();
        let outside = VertexId::new();
        let vertices = vec![
            Vertex {
                id: internal,
                position: Point2::new(60.0, 45.0),
            },
            Vertex {
                id: bottom_left,
                position: Point2::new(10.0, 20.0),
            },
            Vertex {
                id: outside,
                position: Point2::new(-40.0, 95.0),
            },
            Vertex {
                id: top_right,
                position: Point2::new(110.0, 70.0),
            },
            Vertex {
                id: top_left,
                position: Point2::new(10.0, 70.0),
            },
            Vertex {
                id: bottom_right,
                position: Point2::new(110.0, 20.0),
            },
        ];
        // Counter-clockwise and clockwise boundary orders are both valid. Use
        // the clockwise order here to exercise the less common orientation.
        let boundary_vertices = vec![bottom_left, top_left, top_right, bottom_right];
        let edges = vec![
            Edge {
                id: EdgeId::new(),
                start: bottom_left,
                end: top_left,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: top_left,
                end: top_right,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: top_right,
                end: bottom_right,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: bottom_right,
                end: bottom_left,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: EdgeId::new(),
                start: internal,
                end: top_right,
                kind: EdgeKind::Mountain,
            },
        ];
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices,
            thickness_mm: 0.25,
            cutting_allowed: true,
            ..Paper::default()
        };
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
        )
    }

    fn simple_rectangular_editor() -> (EditorState, CreasePattern, Paper) {
        let sheet =
            crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid simple rectangle");
        let (pattern, paper) = sheet.into_parts();
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
        )
    }

    fn collinear_after_removal_editor() -> (EditorState, CreasePattern, Paper, VertexId) {
        let previous = VertexId::new();
        let target = VertexId::new();
        let next = VertexId::new();
        let middle = VertexId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: previous,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(1.0, 1.0),
                },
                Vertex {
                    id: next,
                    position: Point2::new(2.0, 0.0),
                },
                Vertex {
                    id: middle,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![
                Edge {
                    id: EdgeId::new(),
                    start: previous,
                    end: target,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: EdgeId::new(),
                    start: target,
                    end: next,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: EdgeId::new(),
                    start: next,
                    end: middle,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: EdgeId::new(),
                    start: middle,
                    end: previous,
                    kind: EdgeKind::Boundary,
                },
            ],
        };
        let paper = Paper {
            boundary_vertices: vec![previous, target, next, middle],
            ..Paper::default()
        };
        (
            EditorState::with_paper(pattern.clone(), paper.clone()),
            pattern,
            paper,
            target,
        )
    }

    fn assert_split_rejected(editor: &mut EditorState, command: Command, expected: CommandError) {
        let pattern = editor.pattern().clone();
        let paper = editor.paper().clone();
        let error = editor
            .execute(editor.revision(), command)
            .expect_err("boundary split must fail");

        assert_eq!(error, expected);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    fn assert_boundary_removal_rejected(
        editor: &mut EditorState,
        vertex: VertexId,
        expected: CommandError,
    ) {
        let pattern = editor.pattern().clone();
        let paper = editor.paper().clone();
        let error = editor
            .execute(editor.revision(), Command::RemoveBoundaryVertex { vertex })
            .expect_err("boundary vertex removal must fail");

        assert_eq!(error, expected);
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

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
    fn rectangular_paper_resize_scales_every_vertex_and_restores_exactly() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let original_vertex_ids = original_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edges = original_pattern.edges.clone();
        assert!(crate::validate_paper(&original_paper, &original_pattern).is_valid());

        let result = editor
            .execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 200.0,
                    height_mm: 25.0,
                },
            )
            .expect("resize rectangular paper");

        assert_eq!(result.revision, 1);
        assert_eq!(result.changed_vertices, original_vertex_ids);
        assert!(result.changed_edges.is_empty());
        assert!(!result.settings_changed);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.pattern().edges, original_edges);
        assert_eq!(
            editor.pattern().vertices[0].position,
            Point2::new(110.0, 32.5)
        );
        assert_eq!(
            editor.pattern().vertices[1].position,
            Point2::new(10.0, 20.0)
        );
        assert_eq!(
            editor.pattern().vertices[2].position,
            Point2::new(-90.0, 57.5)
        );
        assert_eq!(
            editor.pattern().vertices[3].position,
            Point2::new(210.0, 45.0)
        );
        assert_eq!(
            editor.pattern().vertices[4].position,
            Point2::new(10.0, 45.0)
        );
        assert_eq!(
            editor.pattern().vertices[5].position,
            Point2::new(210.0, 20.0)
        );
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let resized_pattern = editor.pattern().clone();

        let undo = editor.undo(1).expect("undo resize");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, original_vertex_ids);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo resize");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, original_vertex_ids);
        assert_eq!(editor.pattern(), &resized_pattern);
        assert_eq!(editor.paper(), &original_paper);

        editor.undo(3).expect("undo resize again");
        assert_eq!(editor.pattern(), &original_pattern);
        editor.redo(4).expect("redo resize again");
        assert_eq!(editor.pattern(), &resized_pattern);
    }

    #[test]
    fn resizing_to_the_same_dimensions_is_an_exact_undoable_command() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let changed_vertices = original_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();

        let result = editor
            .execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 100.0,
                    height_mm: 50.0,
                },
            )
            .expect("same-size resize");

        assert_eq!(result.revision, 1);
        assert_eq!(result.changed_vertices, changed_vertices);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert!(editor.can_undo());
        editor.undo(1).expect("undo same-size resize");
        assert_eq!(editor.pattern(), &original_pattern);
        editor.redo(2).expect("redo same-size resize");
        assert_eq!(editor.pattern(), &original_pattern);
    }

    #[test]
    fn stale_rectangular_resize_preserves_state_and_history() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();

        let error = editor
            .execute(
                7,
                Command::ResizeRectangularPaper {
                    width_mm: 200.0,
                    height_mm: 100.0,
                },
            )
            .expect_err("stale resize must fail");

        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 7,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn invalid_resize_dimensions_do_not_change_state_or_history() {
        let cases = [
            (f64::NAN, 50.0, CommandError::PaperWidthNotFinite),
            (f64::INFINITY, 50.0, CommandError::PaperWidthNotFinite),
            (0.0, 50.0, CommandError::PaperWidthNotPositive),
            (-1.0, 50.0, CommandError::PaperWidthNotPositive),
            (100.0, f64::NAN, CommandError::PaperHeightNotFinite),
            (100.0, 0.0, CommandError::PaperHeightNotPositive),
            (f64::MAX, 2.0, CommandError::PaperResizeAreaNotRepresentable),
            (
                f64::MIN_POSITIVE,
                f64::MIN_POSITIVE,
                CommandError::PaperResizeAreaNotRepresentable,
            ),
        ];

        for (width_mm, height_mm, expected) in cases {
            let (mut editor, original_pattern, original_paper) = rectangular_editor();
            let error = editor
                .execute(
                    0,
                    Command::ResizeRectangularPaper {
                        width_mm,
                        height_mm,
                    },
                )
                .expect_err("invalid resize must fail");

            assert_eq!(error, expected);
            assert_eq!(editor.pattern(), &original_pattern);
            assert_eq!(editor.paper(), &original_paper);
            assert_eq!(editor.revision(), 0);
            assert!(!editor.can_undo());
            assert!(!editor.can_redo());
        }
    }

    #[test]
    fn invalid_rectangular_boundaries_have_specific_errors_without_mutation() {
        let (_, original_pattern, original_paper) = rectangular_editor();
        let resize = Command::ResizeRectangularPaper {
            width_mm: 200.0,
            height_mm: 100.0,
        };

        let mut count_paper = original_paper.clone();
        count_paper.boundary_vertices.pop();
        let mut editor = EditorState::with_paper(original_pattern.clone(), count_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryVertexCount { actual: 3 })
        );

        let mut duplicate_paper = original_paper.clone();
        let duplicate = duplicate_paper.boundary_vertices[0];
        duplicate_paper.boundary_vertices[3] = duplicate;
        let mut editor = EditorState::with_paper(original_pattern.clone(), duplicate_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryDuplicateVertex { vertex: duplicate })
        );

        let mut missing_paper = original_paper.clone();
        let missing = VertexId::new();
        missing_paper.boundary_vertices[2] = missing;
        let mut editor = EditorState::with_paper(original_pattern.clone(), missing_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryVertexNotFound(
                missing
            ))
        );

        let mut non_finite_pattern = original_pattern.clone();
        let non_finite = original_paper.boundary_vertices[1];
        non_finite_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == non_finite)
            .expect("boundary vertex")
            .position
            .x = f64::NAN;
        let mut editor = EditorState::with_paper(non_finite_pattern, original_paper.clone());
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::RectangularPaperBoundaryPositionNotFinite { vertex: non_finite })
        );

        let mut non_rectangle_pattern = original_pattern.clone();
        let top_right = original_paper.boundary_vertices[2];
        non_rectangle_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == top_right)
            .expect("top right")
            .position
            .x = 100.0;
        let mut editor = EditorState::with_paper(non_rectangle_pattern, original_paper.clone());
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::PaperBoundaryNotRectangle)
        );

        let mut non_adjacent_paper = original_paper.clone();
        non_adjacent_paper.boundary_vertices.swap(1, 2);
        let mut editor = EditorState::with_paper(original_pattern.clone(), non_adjacent_paper);
        assert_eq!(
            editor.execute(0, resize.clone()),
            Err(CommandError::PaperBoundaryVerticesNotAdjacent)
        );

        let diamond_ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let diamond_pattern = CreasePattern {
            vertices: diamond_ids
                .into_iter()
                .zip([
                    Point2::new(0.0, 1.0),
                    Point2::new(1.0, 2.0),
                    Point2::new(2.0, 1.0),
                    Point2::new(1.0, 0.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let diamond_paper = Paper {
            boundary_vertices: diamond_ids.to_vec(),
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(diamond_pattern, diamond_paper);
        assert_eq!(
            editor.execute(0, resize),
            Err(CommandError::PaperBoundaryNotAxisAligned)
        );
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn resize_rejects_unrepresentable_bounds_and_transformed_vertices_atomically() {
        let boundary_ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let anchored_pattern = CreasePattern {
            vertices: boundary_ids
                .into_iter()
                .zip([
                    Point2::new(1.0e308, 0.0),
                    Point2::new(1.1e308, 0.0),
                    Point2::new(1.1e308, 1.0),
                    Point2::new(1.0e308, 1.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let anchored_paper = Paper {
            boundary_vertices: boundary_ids.to_vec(),
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(anchored_pattern.clone(), anchored_paper.clone());
        assert_eq!(
            editor.execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 1.0,
                    height_mm: 1.0,
                }
            ),
            Err(CommandError::PaperResizeBoundaryNotRepresentable)
        );
        assert_eq!(editor.pattern(), &anchored_pattern);
        assert_eq!(editor.paper(), &anchored_paper);
        assert_eq!(editor.revision(), 0);

        let (_, mut pattern, paper) = rectangular_editor();
        pattern.vertices[2].position.x = f64::MAX;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        let overflowing_vertex = pattern.vertices[2].id;
        assert_eq!(
            editor.execute(
                0,
                Command::ResizeRectangularPaper {
                    width_mm: 200.0,
                    height_mm: 50.0,
                }
            ),
            Err(CommandError::PaperResizeVertexPositionNotFinite {
                vertex: overflowing_vertex
            })
        );
        assert_eq!(editor.pattern(), &pattern);
        assert_eq!(editor.paper(), &paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn boundary_split_preserves_ids_order_and_validation_through_undo_redo() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let original_edge = original_pattern.edges[0].clone();
        let new_vertex_id = VertexId::new();
        let new_edge_id = EdgeId::new();
        assert!(crate::validate_paper(&original_paper, &original_pattern).is_valid());

        let result = editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: original_edge.id,
                    new_vertex: new_vertex_id,
                    new_edge: new_edge_id,
                    fraction: 0.25,
                },
            )
            .expect("split boundary edge");

        assert_eq!(result.revision, 1);
        assert_eq!(
            result.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(result.changed_edges, vec![original_edge.id, new_edge_id]);
        assert!(result.settings_changed);
        assert_eq!(
            editor.paper().boundary_vertices,
            vec![
                original_paper.boundary_vertices[0],
                new_vertex_id,
                original_paper.boundary_vertices[1],
                original_paper.boundary_vertices[2],
                original_paper.boundary_vertices[3],
            ]
        );
        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex_id,
                position: Point2::new(10.0, 32.5),
            })
        );
        assert_eq!(editor.pattern().edges[0].id, original_edge.id);
        assert_eq!(editor.pattern().edges[0].start, original_edge.start);
        assert_eq!(editor.pattern().edges[0].end, new_vertex_id);
        assert_eq!(
            editor.pattern().edges[1],
            Edge {
                id: new_edge_id,
                start: new_vertex_id,
                end: original_edge.end,
                kind: EdgeKind::Boundary,
            }
        );
        assert_eq!(editor.pattern().edges[2..], original_pattern.edges[1..]);
        assert_eq!(editor.paper().thickness_mm, original_paper.thickness_mm);
        assert_eq!(
            editor.paper().cutting_allowed,
            original_paper.cutting_allowed
        );
        assert_eq!(editor.paper().front, original_paper.front);
        assert_eq!(editor.paper().back, original_paper.back);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
        let split_pattern = editor.pattern().clone();
        let split_paper = editor.paper().clone();

        let undo = editor.undo(1).expect("undo boundary split");
        assert_eq!(undo.revision, 2);
        assert_eq!(
            undo.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(undo.changed_edges, vec![original_edge.id, new_edge_id]);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo boundary split");
        assert_eq!(redo.revision, 3);
        assert_eq!(
            redo.changed_vertices,
            vec![new_vertex_id, original_edge.start, original_edge.end]
        );
        assert_eq!(editor.pattern(), &split_pattern);
        assert_eq!(editor.paper(), &split_paper);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_split_handles_an_edge_opposite_to_the_paper_order() {
        let (_, mut pattern, paper) = rectangular_editor();
        let forward_edge = pattern.edges[0].clone();
        pattern.edges[0] = Edge {
            start: forward_edge.end,
            end: forward_edge.start,
            ..forward_edge
        };
        let original_edge = pattern.edges[0].clone();
        let new_vertex_id = VertexId::new();
        let new_edge_id = EdgeId::new();
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: original_edge.id,
                    new_vertex: new_vertex_id,
                    new_edge: new_edge_id,
                    fraction: 0.25,
                },
            )
            .expect("split reverse boundary edge");

        assert_eq!(
            editor
                .pattern()
                .vertices
                .last()
                .map(|vertex| vertex.position),
            Some(Point2::new(10.0, 57.5))
        );
        assert_eq!(editor.pattern().edges[0].start, original_edge.start);
        assert_eq!(editor.pattern().edges[0].end, new_vertex_id);
        assert_eq!(editor.pattern().edges[1].start, new_vertex_id);
        assert_eq!(editor.pattern().edges[1].end, original_edge.end);
        assert_eq!(editor.paper().boundary_vertices[1], new_vertex_id);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_split_handles_the_closing_paper_edge() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let original_edge = original_pattern.edges[3].clone();
        let new_vertex_id = VertexId::new();
        let new_edge_id = EdgeId::new();

        editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: original_edge.id,
                    new_vertex: new_vertex_id,
                    new_edge: new_edge_id,
                    fraction: 0.5,
                },
            )
            .expect("split closing edge");

        assert_eq!(editor.paper().boundary_vertices.len(), 5);
        assert_eq!(editor.paper().boundary_vertices[4], new_vertex_id);
        assert_eq!(
            editor
                .pattern()
                .vertices
                .last()
                .map(|vertex| vertex.position),
            Some(Point2::new(60.0, 20.0))
        );
        assert_eq!(editor.pattern().edges[3].id, original_edge.id);
        assert_eq!(editor.pattern().edges[3].start, original_edge.start);
        assert_eq!(editor.pattern().edges[3].end, new_vertex_id);
        assert_eq!(editor.pattern().edges[4].id, new_edge_id);
        assert_eq!(editor.pattern().edges[5], original_pattern.edges[4]);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());

        editor.undo(1).expect("undo closing split");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        editor.redo(2).expect("redo closing split");
        assert_eq!(editor.paper().boundary_vertices[4], new_vertex_id);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_split_uses_a_stable_convex_combination_for_extreme_endpoints() {
        let ids = [VertexId::new(), VertexId::new(), VertexId::new()];
        let edge = Edge {
            id: EdgeId::new(),
            start: ids[0],
            end: ids[1],
            kind: EdgeKind::Boundary,
        };
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: ids[0],
                    position: Point2::new(-f64::MAX, 0.0),
                },
                Vertex {
                    id: ids[1],
                    position: Point2::new(f64::MAX, 0.0),
                },
                Vertex {
                    id: ids[2],
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![edge.clone()],
        };
        let paper = Paper {
            boundary_vertices: ids.to_vec(),
            ..Paper::default()
        };
        let new_vertex = VertexId::new();
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(
                0,
                Command::SplitBoundaryEdge {
                    edge: edge.id,
                    new_vertex,
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            )
            .expect("extreme finite endpoints must interpolate safely");

        assert_eq!(
            editor.pattern().vertices.last(),
            Some(&Vertex {
                id: new_vertex,
                position: Point2::new(0.0, 0.0),
            })
        );
    }

    #[test]
    fn boundary_split_rejects_an_existing_third_vertex_at_the_new_position() {
        let (_, mut pattern, paper) = rectangular_editor();
        let edge = pattern.edges[0].clone();
        let occupied_by = VertexId::new();
        pattern.vertices.push(Vertex {
            id: occupied_by,
            position: Point2::new(10.0, 45.0),
        });
        let mut editor = EditorState::with_paper(pattern, paper);

        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitPositionOccupied {
                vertex: occupied_by,
            },
        );
    }

    #[test]
    fn boundary_split_checks_duplicate_id_vertex_records_for_occupied_positions() {
        let (_, mut pattern, paper) = rectangular_editor();
        let edge = pattern.edges[0].clone();
        pattern.vertices.push(Vertex {
            id: edge.start,
            position: Point2::new(10.0, 45.0),
        });
        let mut editor = EditorState::with_paper(pattern, paper);

        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitPositionOccupied { vertex: edge.start },
        );
    }

    #[test]
    fn invalid_boundary_split_targets_and_ids_are_atomic() {
        let (_, pattern, paper) = rectangular_editor();
        let boundary_edge = pattern.edges[0].clone();
        let mountain_edge = pattern.edges[4].clone();

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: mountain_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::EdgeIsNotBoundary(mountain_edge.id),
        );

        let diagonal = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Boundary,
        };
        let mut diagonal_pattern = pattern.clone();
        diagonal_pattern.edges.push(diagonal.clone());
        let mut editor = EditorState::with_paper(diagonal_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: diagonal.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundaryEdgeNotInPaperBoundary(diagonal.id),
        );

        let mut duplicate_edge_pattern = pattern.clone();
        duplicate_edge_pattern.edges.push(boundary_edge.clone());
        let mut editor = EditorState::with_paper(duplicate_edge_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitTargetEdgeIdAmbiguous {
                edge: boundary_edge.id,
            },
        );

        let first = VertexId::new();
        let second = VertexId::new();
        let ambiguous_edge = Edge {
            id: EdgeId::new(),
            start: first,
            end: second,
            kind: EdgeKind::Boundary,
        };
        let ambiguous_pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![ambiguous_edge.clone()],
        };
        let ambiguous_paper = Paper {
            boundary_vertices: vec![first, second, first],
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(ambiguous_pattern, ambiguous_paper);
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: ambiguous_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundaryEdgeMatchesMultiplePaperSegments {
                edge: ambiguous_edge.id,
            },
        );

        let existing_vertex = pattern.vertices[0].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: existing_vertex,
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::VertexAlreadyExists(existing_vertex),
        );

        let existing_edge = pattern.edges[1].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: VertexId::new(),
                new_edge: existing_edge,
                fraction: 0.5,
            },
            CommandError::EdgeAlreadyExists(existing_edge),
        );
    }

    #[test]
    fn invalid_boundary_split_fractions_positions_and_conflicts_are_atomic() {
        let (_, pattern, paper) = rectangular_editor();
        let boundary_edge = pattern.edges[0].clone();
        for (fraction, expected) in [
            (f64::NAN, CommandError::BoundarySplitFractionNotFinite),
            (f64::INFINITY, CommandError::BoundarySplitFractionNotFinite),
            (0.0, CommandError::BoundarySplitFractionOutOfRange),
            (
                -f64::MIN_POSITIVE,
                CommandError::BoundarySplitFractionOutOfRange,
            ),
            (1.0, CommandError::BoundarySplitFractionOutOfRange),
        ] {
            let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
            assert_split_rejected(
                &mut editor,
                Command::SplitBoundaryEdge {
                    edge: boundary_edge.id,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction,
                },
                expected,
            );
        }

        let mut non_finite_pattern = pattern.clone();
        non_finite_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == boundary_edge.start)
            .expect("boundary start")
            .position
            .x = f64::INFINITY;
        let mut editor = EditorState::with_paper(non_finite_pattern, paper.clone());
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: boundary_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: 0.5,
            },
            CommandError::BoundarySplitEndpointPositionNotFinite {
                edge: boundary_edge.id,
                vertex: boundary_edge.start,
            },
        );

        let close_ids = [VertexId::new(), VertexId::new(), VertexId::new()];
        let close_edge = Edge {
            id: EdgeId::new(),
            start: close_ids[0],
            end: close_ids[1],
            kind: EdgeKind::Boundary,
        };
        let close_pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: close_ids[0],
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: close_ids[1],
                    position: Point2::new(2.0, 0.0),
                },
                Vertex {
                    id: close_ids[2],
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![close_edge.clone()],
        };
        let close_paper = Paper {
            boundary_vertices: close_ids.to_vec(),
            ..Paper::default()
        };
        let mut editor = EditorState::with_paper(close_pattern, close_paper);
        assert_split_rejected(
            &mut editor,
            Command::SplitBoundaryEdge {
                edge: close_edge.id,
                new_vertex: VertexId::new(),
                new_edge: EdgeId::new(),
                fraction: f64::MIN_POSITIVE,
            },
            CommandError::BoundarySplitPositionNotDistinct,
        );

        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        let original_pattern = editor.pattern().clone();
        let original_paper = editor.paper().clone();
        let error = editor
            .execute(
                9,
                Command::SplitBoundaryEdge {
                    edge: boundary_edge.id,
                    new_vertex: VertexId::new(),
                    new_edge: EdgeId::new(),
                    fraction: 0.5,
                },
            )
            .expect_err("stale split must fail");
        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 9,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn boundary_vertex_removal_merges_edges_and_restores_exactly() {
        let (mut editor, original_pattern, original_paper) = simple_rectangular_editor();
        let target = original_paper.boundary_vertices[1];
        let previous = original_paper.boundary_vertices[0];
        let next = original_paper.boundary_vertices[2];
        let kept_edge = original_pattern.edges[0].clone();
        let removed_edge = original_pattern.edges[1].clone();

        let result = editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("remove boundary vertex");

        assert_eq!(result.revision, 1);
        assert_eq!(result.changed_vertices, vec![target, previous, next]);
        assert_eq!(result.changed_edges, vec![kept_edge.id, removed_edge.id]);
        assert!(result.settings_changed);
        let mut expected_pattern = original_pattern.clone();
        expected_pattern
            .vertices
            .retain(|vertex| vertex.id != target);
        expected_pattern.edges[0].end = next;
        expected_pattern.edges.remove(1);
        let mut expected_paper = original_paper.clone();
        expected_paper.boundary_vertices.remove(1);
        assert_eq!(editor.pattern(), &expected_pattern);
        assert_eq!(editor.paper(), &expected_paper);
        assert_eq!(editor.pattern().edges[0].id, kept_edge.id);
        assert_eq!(editor.pattern().edges[0].start, kept_edge.start);
        assert_eq!(editor.pattern().edges[0].end, next);
        assert_eq!(editor.pattern().edges[1..], original_pattern.edges[2..]);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());

        let undo = editor.undo(1).expect("undo boundary vertex removal");
        assert_eq!(undo.revision, 2);
        assert_eq!(undo.changed_vertices, vec![target, previous, next]);
        assert_eq!(undo.changed_edges, vec![kept_edge.id, removed_edge.id]);
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);

        let redo = editor.redo(2).expect("redo boundary vertex removal");
        assert_eq!(redo.revision, 3);
        assert_eq!(redo.changed_vertices, vec![target, previous, next]);
        assert_eq!(editor.pattern(), &expected_pattern);
        assert_eq!(editor.paper(), &expected_paper);
    }

    #[test]
    fn boundary_vertex_removal_preserves_reversed_kept_edge_orientation() {
        let (_, mut pattern, paper) = simple_rectangular_editor();
        let target = paper.boundary_vertices[1];
        let previous = paper.boundary_vertices[0];
        let next = paper.boundary_vertices[2];
        let preceding = pattern.edges[0].clone();
        pattern.edges[0] = Edge {
            start: preceding.end,
            end: preceding.start,
            ..preceding
        };
        let following = pattern.edges[1].clone();
        pattern.edges[1] = Edge {
            start: following.end,
            end: following.start,
            ..following
        };
        let kept_id = pattern.edges[0].id;
        let mut editor = EditorState::with_paper(pattern, paper);

        editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("remove vertex with reverse edges");

        assert_eq!(editor.pattern().edges[0].id, kept_id);
        assert_eq!(editor.pattern().edges[0].start, next);
        assert_eq!(editor.pattern().edges[0].end, previous);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());
    }

    #[test]
    fn boundary_vertex_removal_handles_closing_predecessor_before_exact_undo() {
        let (mut editor, original_pattern, original_paper) = simple_rectangular_editor();
        let target = original_paper.boundary_vertices[0];
        let previous = original_paper.boundary_vertices[3];
        let next = original_paper.boundary_vertices[1];
        let kept_edge = original_pattern.edges[3].clone();
        let removed_edge = original_pattern.edges[0].clone();

        editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("remove vertex at closing boundary junction");

        assert_eq!(
            editor.paper().boundary_vertices,
            original_paper.boundary_vertices[1..]
        );
        assert_eq!(editor.pattern().edges[0], original_pattern.edges[1]);
        assert_eq!(editor.pattern().edges[1], original_pattern.edges[2]);
        assert_eq!(editor.pattern().edges[2].id, kept_edge.id);
        assert_eq!(editor.pattern().edges[2].start, previous);
        assert_eq!(editor.pattern().edges[2].end, next);
        assert!(crate::validate_paper(editor.paper(), editor.pattern()).is_valid());

        editor.undo(1).expect("undo closing vertex removal");
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        editor.redo(2).expect("redo closing vertex removal");
        assert_eq!(editor.pattern().edges[2].id, kept_edge.id);
        assert!(
            !editor
                .pattern()
                .edges
                .iter()
                .any(|edge| edge.id == removed_edge.id)
        );
    }

    #[test]
    fn boundary_vertex_removal_rejects_a_collinear_candidate_from_a_valid_state() {
        let (mut editor, original_pattern, original_paper, target) =
            collinear_after_removal_editor();
        assert!(validate_crease_pattern(&original_pattern).is_valid());
        assert!(validate_paper(&original_paper, &original_pattern).is_valid());

        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRemovalWouldInvalidatePaper,
        );
    }

    #[test]
    fn boundary_vertex_removal_rejects_a_new_edge_through_existing_geometry() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let target = original_paper.boundary_vertices[1];
        assert!(validate_crease_pattern(&original_pattern).is_valid());
        assert!(validate_paper(&original_paper, &original_pattern).is_valid());

        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRemovalWouldInvalidatePaper,
        );
    }

    #[test]
    fn boundary_vertex_removal_can_edit_an_already_invalid_state() {
        let (_, pattern, mut paper, target) = collinear_after_removal_editor();
        paper.thickness_mm = -0.1;
        assert!(validate_crease_pattern(&pattern).is_valid());
        assert!(!validate_paper(&paper, &pattern).is_valid());
        let mut editor = EditorState::with_paper(pattern, paper);

        let result = editor
            .execute(0, Command::RemoveBoundaryVertex { vertex: target })
            .expect("an already invalid project remains editable");

        assert_eq!(result.revision, 1);
        assert!(
            !editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.id == target)
        );
        assert_eq!(editor.paper().boundary_vertices.len(), 3);
    }

    #[test]
    fn boundary_vertex_removal_rejects_invalid_boundary_and_vertex_identity() {
        let (_, pattern, paper) = rectangular_editor();
        let target = paper.boundary_vertices[1];

        let mut triangle_paper = paper.clone();
        triangle_paper.boundary_vertices.pop();
        let mut editor = EditorState::with_paper(pattern.clone(), triangle_paper);
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRemovalNeedsFourVertices { actual: 3 },
        );

        let not_boundary = pattern.vertices[0].id;
        let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            not_boundary,
            CommandError::VertexNotInPaperBoundary(not_boundary),
        );

        let mut duplicate_boundary_paper = paper.clone();
        duplicate_boundary_paper.boundary_vertices[3] = target;
        let mut editor = EditorState::with_paper(pattern.clone(), duplicate_boundary_paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexOccursMultipleTimes { vertex: target },
        );

        let mut missing_pattern = pattern.clone();
        missing_pattern
            .vertices
            .retain(|vertex| vertex.id != target);
        let mut editor = EditorState::with_paper(missing_pattern, paper.clone());
        assert_boundary_removal_rejected(&mut editor, target, CommandError::VertexNotFound(target));

        let mut duplicate_pattern = pattern.clone();
        let duplicate_record = duplicate_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == target)
            .expect("target vertex")
            .clone();
        duplicate_pattern.vertices.push(duplicate_record);
        let mut editor = EditorState::with_paper(duplicate_pattern, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexRecordAmbiguous { vertex: target },
        );

        let previous = paper.boundary_vertices[0];
        let other = paper.boundary_vertices[2];
        let malformed_paper = Paper {
            boundary_vertices: vec![previous, target, previous, other],
            ..paper
        };
        let mut editor = EditorState::with_paper(pattern, malformed_paper);
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexNeighborsNotDistinct {
                vertex: target,
                neighbor: previous,
            },
        );
    }

    #[test]
    fn boundary_vertex_removal_rejects_invalid_adjacent_edge_topology() {
        let (_, pattern, paper) = rectangular_editor();
        let target = paper.boundary_vertices[1];
        let previous = paper.boundary_vertices[0];
        let next = paper.boundary_vertices[2];

        let mut missing_preceding = pattern.clone();
        missing_preceding.edges[0].kind = EdgeKind::Auxiliary;
        let mut editor = EditorState::with_paper(missing_preceding, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexPrecedingEdgeMissing { vertex: target },
        );

        let mut ambiguous_preceding = pattern.clone();
        let mut duplicate_preceding = ambiguous_preceding.edges[0].clone();
        duplicate_preceding.id = EdgeId::new();
        ambiguous_preceding.edges.push(duplicate_preceding);
        let mut editor = EditorState::with_paper(ambiguous_preceding, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexPrecedingEdgeAmbiguous { vertex: target },
        );

        let mut missing_following = pattern.clone();
        missing_following.edges[1].kind = EdgeKind::Valley;
        let mut editor = EditorState::with_paper(missing_following, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexFollowingEdgeMissing { vertex: target },
        );

        let mut duplicate_edge_ids = pattern.clone();
        duplicate_edge_ids.edges[1].id = duplicate_edge_ids.edges[0].id;
        let mut editor = EditorState::with_paper(duplicate_edge_ids, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexAdjacentEdgesNotDistinct { vertex: target },
        );

        for adjacent_edge_id in [pattern.edges[0].id, pattern.edges[1].id] {
            let mut ambiguous_edge_id_pattern = pattern.clone();
            let mut unrelated_record = ambiguous_edge_id_pattern.edges[4].clone();
            unrelated_record.id = adjacent_edge_id;
            ambiguous_edge_id_pattern.edges.push(unrelated_record);
            let mut editor = EditorState::with_paper(ambiguous_edge_id_pattern, paper.clone());
            assert_boundary_removal_rejected(
                &mut editor,
                target,
                CommandError::BoundaryVertexAdjacentEdgeIdAmbiguous {
                    vertex: target,
                    edge: adjacent_edge_id,
                },
            );
        }

        let additional_edge = Edge {
            id: EdgeId::new(),
            start: target,
            end: pattern.vertices[0].id,
            kind: EdgeKind::Mountain,
        };
        let mut additionally_connected = pattern.clone();
        additionally_connected.edges.push(additional_edge.clone());
        let mut editor = EditorState::with_paper(additionally_connected, paper.clone());
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexHasAdditionalEdge {
                vertex: target,
                edge: additional_edge.id,
            },
        );

        let neighbor_edge = Edge {
            id: EdgeId::new(),
            start: previous,
            end: next,
            kind: EdgeKind::Auxiliary,
        };
        let mut already_connected_neighbors = pattern.clone();
        already_connected_neighbors
            .edges
            .push(neighbor_edge.clone());
        let mut editor = EditorState::with_paper(already_connected_neighbors, paper);
        assert_boundary_removal_rejected(
            &mut editor,
            target,
            CommandError::BoundaryVertexNeighborEdgeAlreadyExists {
                vertex: target,
                edge: neighbor_edge.id,
            },
        );
    }

    #[test]
    fn stale_boundary_vertex_removal_preserves_state_and_history() {
        let (mut editor, original_pattern, original_paper) = rectangular_editor();
        let target = original_paper.boundary_vertices[1];

        let error = editor
            .execute(8, Command::RemoveBoundaryVertex { vertex: target })
            .expect_err("stale boundary removal must fail");

        assert_eq!(
            error,
            CommandError::RevisionConflict {
                expected: 8,
                actual: 0,
            }
        );
        assert_eq!(editor.pattern(), &original_pattern);
        assert_eq!(editor.paper(), &original_paper);
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
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
