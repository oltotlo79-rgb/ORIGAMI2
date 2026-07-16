use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, Vertex, VertexId};
use thiserror::Error;

use crate::EditorState;

/// A newly-created single sheet whose boundary and crease pattern agree.
///
/// The fields are kept private so that the four boundary vertex references
/// cannot become detached from the vertices and boundary edges while this
/// value is being passed between project-creation layers.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetProject {
    pattern: CreasePattern,
    paper: Paper,
}

impl SheetProject {
    /// Returns the crease pattern containing the sheet boundary.
    #[must_use]
    pub const fn pattern(&self) -> &CreasePattern {
        &self.pattern
    }

    /// Returns the physical and visual paper settings.
    #[must_use]
    pub const fn paper(&self) -> &Paper {
        &self.paper
    }

    /// Returns the boundary vertex IDs in clockwise canvas order from the
    /// top-left, with positive Y pointing down the canvas.
    ///
    /// A rectangular sheet always owns exactly four boundary vertices, so an
    /// array communicates that invariant to callers without requiring them to
    /// inspect the paper's vector representation.
    #[must_use]
    pub fn boundary_vertex_ids(&self) -> [VertexId; 4] {
        [
            self.paper.boundary_vertices[0],
            self.paper.boundary_vertices[1],
            self.paper.boundary_vertices[2],
            self.paper.boundary_vertices[3],
        ]
    }

    /// Creates an editor snapshot without adding project creation to history.
    ///
    /// The sheet remains available to the caller. Use
    /// [`Self::into_editor_state`] when retaining it is unnecessary.
    #[must_use]
    pub fn editor_state(&self) -> EditorState {
        EditorState::with_paper(self.pattern.clone(), self.paper.clone())
    }

    /// Consumes this sheet and creates an editor without undo or redo history.
    #[must_use]
    pub fn into_editor_state(self) -> EditorState {
        EditorState::with_paper(self.pattern, self.paper)
    }

    /// Separates the generated crease pattern and paper metadata for
    /// persistence or integration into a larger project aggregate.
    #[must_use]
    pub fn into_parts(self) -> (CreasePattern, Paper) {
        (self.pattern, self.paper)
    }
}

/// Reports which rectangular sheet dimension failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SheetCreationError {
    #[error("sheet width must be finite")]
    WidthNotFinite,
    #[error("sheet width must be greater than zero")]
    WidthNotPositive,
    #[error("sheet height must be finite")]
    HeightNotFinite,
    #[error("sheet height must be greater than zero")]
    HeightNotPositive,
    #[error("sheet dimensions must form a finite, non-zero geometric area")]
    AreaNotRepresentable,
}

/// Creates one rectangular sheet in millimetres with its top-left at `(0, 0)`.
///
/// Boundary vertices are ordered clockwise in canvas coordinates (positive Y
/// points down): top-left, top-right, bottom-right, and bottom-left. Four
/// boundary edges connect the same sequence and close the rectangle. Paper
/// thickness and front/back appearances use domain defaults; `cutting_allowed`
/// is stored both for persistence and subsequent editor creation.
pub fn create_rectangular_sheet(
    width_mm: f64,
    height_mm: f64,
    cutting_allowed: bool,
) -> Result<SheetProject, SheetCreationError> {
    validate_dimensions(width_mm, height_mm)?;

    let top_left = VertexId::new();
    let top_right = VertexId::new();
    let bottom_right = VertexId::new();
    let bottom_left = VertexId::new();
    let boundary_vertices = [top_left, top_right, bottom_right, bottom_left];

    let vertices = vec![
        Vertex {
            id: top_left,
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: top_right,
            position: Point2::new(width_mm, 0.0),
        },
        Vertex {
            id: bottom_right,
            position: Point2::new(width_mm, height_mm),
        },
        Vertex {
            id: bottom_left,
            position: Point2::new(0.0, height_mm),
        },
    ];
    let edges = vec![
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
            start: bottom_left,
            end: top_left,
            kind: EdgeKind::Boundary,
        },
    ];
    let paper = Paper {
        boundary_vertices: boundary_vertices.into(),
        cutting_allowed,
        ..Paper::default()
    };

    Ok(SheetProject {
        pattern: CreasePattern { vertices, edges },
        paper,
    })
}

fn validate_dimensions(width_mm: f64, height_mm: f64) -> Result<(), SheetCreationError> {
    if !width_mm.is_finite() {
        return Err(SheetCreationError::WidthNotFinite);
    }
    if width_mm <= 0.0 {
        return Err(SheetCreationError::WidthNotPositive);
    }
    if !height_mm.is_finite() {
        return Err(SheetCreationError::HeightNotFinite);
    }
    if height_mm <= 0.0 {
        return Err(SheetCreationError::HeightNotPositive);
    }

    // The validators and folding engine use the rectangle's signed double
    // area. Reject dimensions that are individually finite but whose product
    // underflows to zero or whose double area overflows.
    let area = width_mm * height_mm;
    let double_area = area + area;
    if area == 0.0 || !double_area.is_finite() {
        return Err(SheetCreationError::AreaNotRepresentable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use ori_domain::{
        DEFAULT_PAPER_BACK_COLOR, DEFAULT_PAPER_FRONT_COLOR, DEFAULT_PAPER_THICKNESS_MM,
    };
    use ori_geometry::{validate_crease_pattern, validate_paper};

    use super::*;

    #[test]
    fn rectangle_has_ordered_vertices_and_closed_boundary_edges() {
        let sheet = create_rectangular_sheet(210.0, 297.0, false).expect("valid A4 sheet");
        let ids = sheet.boundary_vertex_ids();

        assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), 4);
        assert_eq!(sheet.paper().boundary_vertices.as_slice(), &ids);
        assert_eq!(
            sheet
                .pattern()
                .vertices
                .iter()
                .map(|vertex| (vertex.id, vertex.position))
                .collect::<Vec<_>>(),
            vec![
                (ids[0], Point2::new(0.0, 0.0)),
                (ids[1], Point2::new(210.0, 0.0)),
                (ids[2], Point2::new(210.0, 297.0)),
                (ids[3], Point2::new(0.0, 297.0)),
            ]
        );
        assert_eq!(sheet.pattern().edges.len(), 4);
        for (edge, expected) in sheet.pattern().edges.iter().zip([
            (ids[0], ids[1]),
            (ids[1], ids[2]),
            (ids[2], ids[3]),
            (ids[3], ids[0]),
        ]) {
            assert_eq!((edge.start, edge.end), expected);
            assert_eq!(edge.kind, EdgeKind::Boundary);
        }
        assert!(validate_crease_pattern(sheet.pattern()).is_valid());
        assert!(validate_paper(sheet.paper(), sheet.pattern()).is_valid());
    }

    #[test]
    fn rectangle_uses_default_paper_properties_and_requested_cutting_policy() {
        let without_cutting = create_rectangular_sheet(150.0, 150.0, false).expect("valid square");
        let with_cutting = create_rectangular_sheet(150.0, 150.0, true).expect("valid square");

        assert_eq!(
            without_cutting.paper().thickness_mm,
            DEFAULT_PAPER_THICKNESS_MM
        );
        assert_eq!(
            without_cutting.paper().front.color,
            DEFAULT_PAPER_FRONT_COLOR
        );
        assert_eq!(without_cutting.paper().back.color, DEFAULT_PAPER_BACK_COLOR);
        assert_eq!(without_cutting.paper().front.texture_asset, None);
        assert_eq!(without_cutting.paper().back.texture_asset, None);
        assert!(!without_cutting.paper().cutting_allowed);
        assert!(with_cutting.paper().cutting_allowed);
    }

    #[test]
    fn editor_starts_with_sheet_and_settings_but_no_history() {
        let sheet = create_rectangular_sheet(100.0, 80.0, true).expect("valid rectangle");
        let expected_pattern = sheet.pattern().clone();
        let expected_paper = sheet.paper().clone();
        let editor = sheet.editor_state();

        assert_eq!(editor.pattern(), &expected_pattern);
        assert_eq!(editor.paper(), &expected_paper);
        assert!(editor.cutting_allowed());
        assert_eq!(editor.revision(), 0);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());

        let consumed_editor = sheet.into_editor_state();
        assert_eq!(consumed_editor.pattern(), &expected_pattern);
        assert_eq!(consumed_editor.paper(), &expected_paper);
        assert!(consumed_editor.cutting_allowed());
        assert_eq!(consumed_editor.revision(), 0);
        assert!(!consumed_editor.can_undo());
        assert!(!consumed_editor.can_redo());
    }

    #[test]
    fn rejects_every_invalid_width_category() {
        for width in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                create_rectangular_sheet(width, 1.0, false),
                Err(SheetCreationError::WidthNotFinite)
            );
        }
        for width in [0.0, -0.0, -1.0] {
            assert_eq!(
                create_rectangular_sheet(width, 1.0, false),
                Err(SheetCreationError::WidthNotPositive)
            );
        }
    }

    #[test]
    fn rejects_every_invalid_height_category() {
        for height in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                create_rectangular_sheet(1.0, height, false),
                Err(SheetCreationError::HeightNotFinite)
            );
        }
        for height in [0.0, -0.0, -1.0] {
            assert_eq!(
                create_rectangular_sheet(1.0, height, false),
                Err(SheetCreationError::HeightNotPositive)
            );
        }
    }

    #[test]
    fn rejects_dimension_combinations_with_unrepresentable_area() {
        assert_eq!(
            create_rectangular_sheet(f64::MAX, f64::MAX, false),
            Err(SheetCreationError::AreaNotRepresentable)
        );
        assert_eq!(
            create_rectangular_sheet(f64::MAX, 1.0, false),
            Err(SheetCreationError::AreaNotRepresentable)
        );
        assert_eq!(
            create_rectangular_sheet(f64::MIN_POSITIVE, f64::MIN_POSITIVE, false),
            Err(SheetCreationError::AreaNotRepresentable)
        );
    }
}
