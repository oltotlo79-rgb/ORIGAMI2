//! Strict, bounded import of the supported two-dimensional FOLD subset.
//!
//! FOLD is a JSON interchange format whose fields are parallel arrays. This
//! adapter intentionally separates untrusted-file parsing from the policy
//! decisions needed to create an ORIGAMI2 crease pattern. Parsing produces a
//! preview with source indices; conversion happens only after the caller
//! supplies an explicit millimetre scale and assignment mapping.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    marker::PhantomData,
};

use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};
use ori_geometry::{
    Orientation, SegmentIntersection, exact_polygon_orientation, segment_intersection,
};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeSeed, Error as DeserializeError, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use thiserror::Error;

/// Largest supported FOLD specification version.
pub const MAX_SUPPORTED_FOLD_SPEC: f64 = 1.2;
/// Maximum title length accepted into the preview.
pub const MAX_FOLD_TITLE_CHARS: usize = 512;
/// Maximum custom unit length accepted into the preview.
pub const MAX_FOLD_UNIT_CHARS: usize = 64;
const MAX_FOLD_METADATA_ITEMS: usize = 64;
const MAX_FOLD_TOP_LEVEL_FIELDS: usize = 256;
const MAX_FOLD_INTERSECTION_CANDIDATES: usize = 1_000_000;

/// Resource limits applied before a FOLD preview is returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoldImportLimits {
    pub max_file_bytes: usize,
    pub max_vertices: usize,
    pub max_edges: usize,
    pub max_boundary_edges: usize,
    pub max_intersection_candidates: usize,
}

impl Default for FoldImportLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 16 * 1024 * 1024,
            max_vertices: 10_000,
            max_edges: 10_000,
            // Boundary validation compares every non-adjacent pair. 1,414
            // edges keep that exact work below one million pairs.
            max_boundary_edges: 1_414,
            max_intersection_candidates: MAX_FOLD_INTERSECTION_CANDIDATES,
        }
    }
}

/// FOLD edge assignment tokens defined by specification 1.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FoldEdgeAssignment {
    #[serde(rename = "B")]
    Boundary,
    #[serde(rename = "M")]
    Mountain,
    #[serde(rename = "V")]
    Valley,
    #[serde(rename = "F")]
    Flat,
    #[serde(rename = "U")]
    Unassigned,
    #[serde(rename = "C")]
    Cut,
    #[serde(rename = "J")]
    Join,
}

impl FoldEdgeAssignment {
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Boundary => "B",
            Self::Mountain => "M",
            Self::Valley => "V",
            Self::Flat => "F",
            Self::Unassigned => "U",
            Self::Cut => "C",
            Self::Join => "J",
        }
    }
}

/// Unit declared by `frame_unit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FoldFrameUnit {
    Unspecified,
    Unitless,
    Inch,
    Point,
    Metre,
    Centimetre,
    Millimetre,
    Micrometre,
    Nanometre,
    Custom(String),
}

impl FoldFrameUnit {
    /// Returns the specification-defined scale when the unit has physical
    /// meaning. Unitless, unspecified and custom units require user choice.
    #[must_use]
    pub fn millimetres_per_unit(&self) -> Option<f64> {
        match self {
            Self::Inch => Some(25.4),
            Self::Point => Some(25.4 / 72.0),
            Self::Metre => Some(1_000.0),
            Self::Centimetre => Some(10.0),
            Self::Millimetre => Some(1.0),
            Self::Micrometre => Some(0.001),
            Self::Nanometre => Some(0.000_001),
            Self::Unspecified | Self::Unitless | Self::Custom(_) => None,
        }
    }
}

/// One source vertex retained for preview rendering and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FoldPreviewVertex {
    pub index: usize,
    pub position: Point2,
}

/// One source edge retained for preview rendering and mapping selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FoldPreviewEdge {
    pub index: usize,
    pub vertices: [usize; 2],
    pub assignment: FoldEdgeAssignment,
}

/// Counts used to show only relevant assignment mapping controls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct FoldAssignmentCounts {
    pub boundary: usize,
    pub mountain: usize,
    pub valley: usize,
    pub flat: usize,
    pub unassigned: usize,
    pub cut: usize,
    pub join: usize,
}

impl FoldAssignmentCounts {
    fn add(&mut self, assignment: FoldEdgeAssignment) {
        let count = match assignment {
            FoldEdgeAssignment::Boundary => &mut self.boundary,
            FoldEdgeAssignment::Mountain => &mut self.mountain,
            FoldEdgeAssignment::Valley => &mut self.valley,
            FoldEdgeAssignment::Flat => &mut self.flat,
            FoldEdgeAssignment::Unassigned => &mut self.unassigned,
            FoldEdgeAssignment::Cut => &mut self.cut,
            FoldEdgeAssignment::Join => &mut self.join,
        };
        *count += 1;
    }

    fn count(self, assignment: FoldEdgeAssignment) -> usize {
        match assignment {
            FoldEdgeAssignment::Boundary => self.boundary,
            FoldEdgeAssignment::Mountain => self.mountain,
            FoldEdgeAssignment::Valley => self.valley,
            FoldEdgeAssignment::Flat => self.flat,
            FoldEdgeAssignment::Unassigned => self.unassigned,
            FoldEdgeAssignment::Cut => self.cut,
            FoldEdgeAssignment::Join => self.join,
        }
    }
}

/// Non-fatal facts shown before applying the import.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FoldPreviewWarning {
    MissingFileSpec,
    UnitNeedsScaleSelection,
    IgnoredFields { names: Vec<String> },
}

/// Validated, immutable FOLD geometry with original zero-based indices.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FoldPreview {
    file_spec: Option<f64>,
    title: Option<String>,
    frame_unit: FoldFrameUnit,
    vertices: Vec<FoldPreviewVertex>,
    edges: Vec<FoldPreviewEdge>,
    boundary_vertex_indices: Vec<usize>,
    assignment_counts: FoldAssignmentCounts,
    warnings: Vec<FoldPreviewWarning>,
}

impl FoldPreview {
    #[must_use]
    pub const fn file_spec(&self) -> Option<f64> {
        self.file_spec
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub const fn frame_unit(&self) -> &FoldFrameUnit {
        &self.frame_unit
    }

    #[must_use]
    pub fn recommended_millimetres_per_unit(&self) -> Option<f64> {
        self.frame_unit.millimetres_per_unit()
    }

    #[must_use]
    pub fn vertices(&self) -> &[FoldPreviewVertex] {
        &self.vertices
    }

    #[must_use]
    pub fn edges(&self) -> &[FoldPreviewEdge] {
        &self.edges
    }

    #[must_use]
    pub fn boundary_vertex_indices(&self) -> &[usize] {
        &self.boundary_vertex_indices
    }

    #[must_use]
    pub const fn assignment_counts(&self) -> FoldAssignmentCounts {
        self.assignment_counts
    }

    #[must_use]
    pub fn warnings(&self) -> &[FoldPreviewWarning] {
        &self.warnings
    }

    /// Applies caller-confirmed unit and assignment policy.
    pub fn convert(
        &self,
        options: &FoldConversionOptions,
    ) -> Result<FoldCreasePatternConversion, FoldConversionError> {
        convert_preview(self, options)
    }
}

/// Action chosen for one FOLD assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum FoldAssignmentTarget {
    ImportAs { edge_kind: EdgeKind },
    Ignore,
}

/// Explicit mapping. `None` is never guessed when that assignment occurs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoldAssignmentMapping {
    pub boundary: Option<FoldAssignmentTarget>,
    pub mountain: Option<FoldAssignmentTarget>,
    pub valley: Option<FoldAssignmentTarget>,
    pub flat: Option<FoldAssignmentTarget>,
    pub unassigned: Option<FoldAssignmentTarget>,
    pub cut: Option<FoldAssignmentTarget>,
    pub join: Option<FoldAssignmentTarget>,
}

impl FoldAssignmentMapping {
    fn target(self, assignment: FoldEdgeAssignment) -> Option<FoldAssignmentTarget> {
        match assignment {
            FoldEdgeAssignment::Boundary => self.boundary,
            FoldEdgeAssignment::Mountain => self.mountain,
            FoldEdgeAssignment::Valley => self.valley,
            FoldEdgeAssignment::Flat => self.flat,
            FoldEdgeAssignment::Unassigned => self.unassigned,
            FoldEdgeAssignment::Cut => self.cut,
            FoldEdgeAssignment::Join => self.join,
        }
    }
}

/// All decisions required to leave preview mode.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FoldConversionOptions {
    pub assignment_mapping: FoldAssignmentMapping,
    pub millimetres_per_unit: f64,
}

/// Converted pattern plus stable source-index-to-UUID correspondence.
#[derive(Debug, Clone, PartialEq)]
pub struct FoldCreasePatternConversion {
    crease_pattern: CreasePattern,
    vertex_ids: Vec<VertexId>,
    edge_ids: Vec<Option<EdgeId>>,
    boundary_vertices: Vec<VertexId>,
}

impl FoldCreasePatternConversion {
    #[must_use]
    pub const fn crease_pattern(&self) -> &CreasePattern {
        &self.crease_pattern
    }

    #[must_use]
    pub fn vertex_ids(&self) -> &[VertexId] {
        &self.vertex_ids
    }

    #[must_use]
    pub fn edge_ids(&self) -> &[Option<EdgeId>] {
        &self.edge_ids
    }

    #[must_use]
    pub fn boundary_vertices(&self) -> &[VertexId] {
        &self.boundary_vertices
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CreasePattern,
        Vec<VertexId>,
        Vec<Option<EdgeId>>,
        Vec<VertexId>,
    ) {
        (
            self.crease_pattern,
            self.vertex_ids,
            self.edge_ids,
            self.boundary_vertices,
        )
    }
}

#[derive(Debug)]
struct RawFold {
    file_spec: Option<f64>,
    file_title: Option<String>,
    frame_title: Option<String>,
    frame_classes: Option<Vec<String>>,
    frame_attributes: Option<Vec<String>>,
    frame_unit: Option<String>,
    vertices_coords: Vec<RawFoldCoordinate>,
    edges_vertices: Vec<[usize; 2]>,
    edges_assignment: Vec<FoldEdgeAssignment>,
    other: BTreeMap<String, IgnoredJsonValue>,
}

struct RawFoldSeed {
    limits: FoldImportLimits,
}

impl<'de> DeserializeSeed<'de> for RawFoldSeed {
    type Value = RawFold;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(RawFoldVisitor {
            limits: self.limits,
        })
    }
}

struct RawFoldVisitor {
    limits: FoldImportLimits,
}

impl<'de> Visitor<'de> for RawFoldVisitor {
    type Value = RawFold;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a root FOLD JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut file_spec = None;
        let mut file_title = None;
        let mut frame_title = None;
        let mut frame_classes = None;
        let mut frame_attributes = None;
        let mut frame_unit = None;
        let mut vertices_coords = None;
        let mut edges_vertices = None;
        let mut edges_assignment = None;
        let mut other = BTreeMap::new();
        let mut field_count = 0_usize;

        while let Some(key) = map.next_key::<String>()? {
            field_count = field_count.saturating_add(1);
            if field_count > MAX_FOLD_TOP_LEVEL_FIELDS {
                return Err(A::Error::custom(format_args!(
                    "FOLD has more than {MAX_FOLD_TOP_LEVEL_FIELDS} top-level fields"
                )));
            }
            match key.as_str() {
                "file_spec" => {
                    if file_spec.is_some() {
                        return Err(A::Error::duplicate_field("file_spec"));
                    }
                    file_spec = Some(map.next_value::<f64>()?);
                }
                "file_title" => {
                    if file_title.is_some() {
                        return Err(A::Error::duplicate_field("file_title"));
                    }
                    file_title = Some(map.next_value::<String>()?);
                }
                "frame_title" => {
                    if frame_title.is_some() {
                        return Err(A::Error::duplicate_field("frame_title"));
                    }
                    frame_title = Some(map.next_value::<String>()?);
                }
                "frame_classes" => {
                    if frame_classes.is_some() {
                        return Err(A::Error::duplicate_field("frame_classes"));
                    }
                    let values = map.next_value_seed(BoundedVecSeed::<String>::new(
                        MAX_FOLD_METADATA_ITEMS,
                        "frame_classes",
                    ))?;
                    if values.len() > MAX_FOLD_METADATA_ITEMS {
                        return Err(A::Error::custom(format_args!(
                            "FOLD frame_classes has more than {MAX_FOLD_METADATA_ITEMS} entries"
                        )));
                    }
                    frame_classes = Some(values);
                }
                "frame_attributes" => {
                    if frame_attributes.is_some() {
                        return Err(A::Error::duplicate_field("frame_attributes"));
                    }
                    let values = map.next_value_seed(BoundedVecSeed::<String>::new(
                        MAX_FOLD_METADATA_ITEMS,
                        "frame_attributes",
                    ))?;
                    if values.len() > MAX_FOLD_METADATA_ITEMS {
                        return Err(A::Error::custom(format_args!(
                            "FOLD frame_attributes has more than {MAX_FOLD_METADATA_ITEMS} entries"
                        )));
                    }
                    frame_attributes = Some(values);
                }
                "frame_unit" => {
                    if frame_unit.is_some() {
                        return Err(A::Error::duplicate_field("frame_unit"));
                    }
                    frame_unit = Some(map.next_value::<String>()?);
                }
                "vertices_coords" => {
                    if vertices_coords.is_some() {
                        return Err(A::Error::duplicate_field("vertices_coords"));
                    }
                    vertices_coords = Some(map.next_value_seed(BoundedVecSeed::<
                        RawFoldCoordinate,
                    >::new(
                        self.limits.max_vertices,
                        "vertices_coords",
                    ))?);
                }
                "edges_vertices" => {
                    if edges_vertices.is_some() {
                        return Err(A::Error::duplicate_field("edges_vertices"));
                    }
                    edges_vertices = Some(map.next_value_seed(
                        BoundedVecSeed::<[usize; 2]>::new(self.limits.max_edges, "edges_vertices"),
                    )?);
                }
                "edges_assignment" => {
                    if edges_assignment.is_some() {
                        return Err(A::Error::duplicate_field("edges_assignment"));
                    }
                    edges_assignment = Some(map.next_value_seed(BoundedVecSeed::<
                        FoldEdgeAssignment,
                    >::new(
                        self.limits.max_edges,
                        "edges_assignment",
                    ))?);
                }
                _ => {
                    if other.contains_key(&key) {
                        return Err(A::Error::custom(format_args!(
                            "duplicate FOLD field {key:?}"
                        )));
                    }
                    map.next_value::<IgnoredAny>()?;
                    other.insert(key, IgnoredJsonValue);
                }
            }
        }

        Ok(RawFold {
            file_spec,
            file_title,
            frame_title,
            frame_classes,
            frame_attributes,
            frame_unit,
            vertices_coords: vertices_coords
                .ok_or_else(|| A::Error::missing_field("vertices_coords"))?,
            edges_vertices: edges_vertices
                .ok_or_else(|| A::Error::missing_field("edges_vertices"))?,
            edges_assignment: edges_assignment
                .ok_or_else(|| A::Error::missing_field("edges_assignment"))?,
            other,
        })
    }
}

struct BoundedVecSeed<T> {
    maximum: usize,
    field: &'static str,
    marker: PhantomData<T>,
}

impl<T> BoundedVecSeed<T> {
    const fn new(maximum: usize, field: &'static str) -> Self {
        Self {
            maximum,
            field,
            marker: PhantomData,
        }
    }
}

impl<'de, T> DeserializeSeed<'de> for BoundedVecSeed<T>
where
    T: Deserialize<'de>,
{
    type Value = Vec<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedVecVisitor {
            maximum: self.maximum,
            field: self.field,
            marker: PhantomData,
        })
    }
}

struct BoundedVecVisitor<T> {
    maximum: usize,
    field: &'static str,
    marker: PhantomData<T>,
}

impl<'de, T> Visitor<'de> for BoundedVecVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "a bounded FOLD {} array", self.field)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let retained_limit = self.maximum.saturating_add(1);
        let capacity = sequence.size_hint().unwrap_or(0).min(retained_limit);
        let mut values = Vec::with_capacity(capacity);
        while values.len() < retained_limit {
            let Some(value) = sequence.next_element::<T>()? else {
                return Ok(values);
            };
            values.push(value);
        }
        while sequence.next_element::<IgnoredAny>()?.is_some() {}
        Ok(values)
    }
}

#[derive(Debug)]
struct RawFoldCoordinate {
    values: [f64; 3],
    dimensions: usize,
}

impl<'de> Deserialize<'de> for RawFoldCoordinate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CoordinateVisitor;

        impl<'de> Visitor<'de> for CoordinateVisitor {
            type Value = RawFoldCoordinate;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a FOLD coordinate array")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = [0.0; 3];
                let mut dimensions = 0;
                while let Some(value) = sequence.next_element::<f64>()? {
                    if dimensions < values.len() {
                        values[dimensions] = value;
                    }
                    dimensions += 1;
                }
                Ok(RawFoldCoordinate { values, dimensions })
            }
        }

        deserializer.deserialize_seq(CoordinateVisitor)
    }
}

#[derive(Debug)]
struct IgnoredJsonValue;

/// Errors raised while reading untrusted FOLD bytes.
#[derive(Debug, Error)]
pub enum FoldImportError {
    #[error("FOLD data is {actual} bytes; the limit is {maximum} bytes")]
    FileTooLarge { actual: usize, maximum: usize },
    #[error("FOLD JSON or supported field structure is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("FOLD file_spec must be finite and positive")]
    InvalidFileSpec,
    #[error("FOLD file_spec {found} is unsupported; supported versions are 1, 1.1 and 1.2")]
    UnsupportedFileSpec { found: f64 },
    #[error("FOLD frame class {class:?} is not a two-dimensional crease pattern")]
    UnsupportedFrameClass { class: String },
    #[error("FOLD frame attribute {attribute:?} is not supported by the 2D importer")]
    UnsupportedFrameAttribute { attribute: String },
    #[error("FOLD title is {actual} characters; the limit is {maximum}")]
    TitleTooLong { actual: usize, maximum: usize },
    #[error("FOLD custom frame_unit is invalid")]
    InvalidFrameUnit,
    #[error("FOLD has {actual} vertices; the limit is {maximum}")]
    TooManyVertices { actual: usize, maximum: usize },
    #[error("FOLD has {actual} edges; the limit is {maximum}")]
    TooManyEdges { actual: usize, maximum: usize },
    #[error("FOLD vertex {vertex_index} has non-finite coordinates")]
    NonFiniteCoordinate { vertex_index: usize },
    #[error(
        "FOLD vertex {vertex_index} has {dimensions} coordinates; only 2D or explicit zero-z 3D coordinates are supported"
    )]
    UnsupportedCoordinateDimension {
        vertex_index: usize,
        dimensions: usize,
    },
    #[error(
        "FOLD vertex {vertex_index} has non-zero z coordinate {z}; folded or 3D geometry is unsupported"
    )]
    NonPlanarCoordinate { vertex_index: usize, z: f64 },
    #[error("FOLD vertices {first_index} and {duplicate_index} have duplicate coordinates")]
    DuplicateVertex {
        first_index: usize,
        duplicate_index: usize,
    },
    #[error("edges_assignment has {assignments} entries for {edges} edges")]
    AssignmentCountMismatch { edges: usize, assignments: usize },
    #[error(
        "FOLD edge {edge_index} references vertex {vertex_index}, but only {vertex_count} exist"
    )]
    VertexIndexOutOfBounds {
        edge_index: usize,
        vertex_index: usize,
        vertex_count: usize,
    },
    #[error("FOLD edge {edge_index} is degenerate")]
    DegenerateEdge { edge_index: usize },
    #[error("FOLD edge {edge_index} has an unrepresentable coordinate span")]
    EdgeSpanNotRepresentable { edge_index: usize },
    #[error("FOLD edges {first_index} and {duplicate_index} duplicate the same segment")]
    DuplicateEdge {
        first_index: usize,
        duplicate_index: usize,
    },
    #[error("FOLD has {actual} boundary edges; the limit is {maximum}")]
    TooManyBoundaryEdges { actual: usize, maximum: usize },
    #[error(
        "FOLD geometry has more than {maximum} broad-phase intersection candidates; validation work is bounded"
    )]
    TooManyIntersectionCandidates { maximum: usize },
    #[error("FOLD boundary requires at least three B edges, found {actual}")]
    BoundaryTooSmall { actual: usize },
    #[error("FOLD boundary vertex {vertex_index} has degree {degree}, expected 2")]
    BoundaryDegree { vertex_index: usize, degree: usize },
    #[error("FOLD B edges do not form one closed cycle")]
    BoundaryDisconnected,
    #[error("FOLD boundary has zero geometric area")]
    BoundaryZeroArea,
    #[error("FOLD boundary edges {first_edge} and {second_edge} intersect")]
    BoundarySelfIntersection {
        first_edge: usize,
        second_edge: usize,
    },
    #[error("FOLD boundary geometry cannot be classified safely")]
    BoundaryGeometryUnrepresentable,
    #[error("FOLD assignment {assignment:?} requires file_spec 1.2")]
    AssignmentRequiresSpec12 { assignment: FoldEdgeAssignment },
}

/// Errors raised only after the user confirms import policy.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum FoldConversionError {
    #[error("millimetres_per_unit must be finite and greater than zero")]
    InvalidMillimetresPerUnit,
    #[error("no mapping was selected for FOLD assignment {assignment:?}")]
    MappingNotSpecified { assignment: FoldEdgeAssignment },
    #[error("mapping {target:?} is not permitted for FOLD assignment {assignment:?}")]
    InvalidMapping {
        assignment: FoldEdgeAssignment,
        target: FoldAssignmentTarget,
    },
    #[error("scaled coordinate for vertex {vertex_index} is not finite")]
    ScaledCoordinateNotFinite { vertex_index: usize },
    #[error("scaling collapses vertices {first_index} and {duplicate_index}")]
    ScaledVertexCollision {
        first_index: usize,
        duplicate_index: usize,
    },
    #[error("scaled FOLD geometry has more than {maximum} broad-phase intersection candidates")]
    TooManyIntersectionCandidates { maximum: usize },
}

/// Reads a preview with conservative desktop defaults.
pub fn read_fold_preview(bytes: &[u8]) -> Result<FoldPreview, FoldImportError> {
    read_fold_preview_with_limits(bytes, FoldImportLimits::default())
}

/// Reads only the supported root-frame 2D fields and validates them.
pub fn read_fold_preview_with_limits(
    bytes: &[u8],
    limits: FoldImportLimits,
) -> Result<FoldPreview, FoldImportError> {
    if bytes.len() > limits.max_file_bytes {
        return Err(FoldImportError::FileTooLarge {
            actual: bytes.len(),
            maximum: limits.max_file_bytes,
        });
    }
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let raw = RawFoldSeed { limits }.deserialize(&mut deserializer)?;
    deserializer.end()?;
    if raw.vertices_coords.len() > limits.max_vertices {
        return Err(FoldImportError::TooManyVertices {
            actual: raw.vertices_coords.len(),
            maximum: limits.max_vertices,
        });
    }
    if raw.edges_vertices.len() > limits.max_edges {
        return Err(FoldImportError::TooManyEdges {
            actual: raw.edges_vertices.len(),
            maximum: limits.max_edges,
        });
    }
    if raw.edges_assignment.len() != raw.edges_vertices.len() {
        return Err(FoldImportError::AssignmentCountMismatch {
            edges: raw.edges_vertices.len(),
            assignments: raw.edges_assignment.len(),
        });
    }
    if let Some(file_spec) = raw.file_spec {
        if !file_spec.is_finite() || file_spec <= 0.0 {
            return Err(FoldImportError::InvalidFileSpec);
        }
        if file_spec != 1.0 && file_spec != 1.1 && file_spec != MAX_SUPPORTED_FOLD_SPEC {
            return Err(FoldImportError::UnsupportedFileSpec { found: file_spec });
        }
    }
    if let Some(classes) = &raw.frame_classes
        && classes.iter().any(|class| class == "foldedForm")
    {
        return Err(FoldImportError::UnsupportedFrameClass {
            class: "foldedForm".to_owned(),
        });
    }
    if let Some(attributes) = &raw.frame_attributes
        && attributes.iter().any(|attribute| attribute == "3D")
    {
        return Err(FoldImportError::UnsupportedFrameAttribute {
            attribute: "3D".to_owned(),
        });
    }

    let mut ignored_field_names = raw.other.keys().cloned().collect::<Vec<_>>();
    if raw.file_title.is_some() && raw.frame_title.is_some() {
        ignored_field_names.push("frame_title".to_owned());
    }
    if raw
        .frame_classes
        .as_ref()
        .is_some_and(|classes| classes.iter().any(|class| class != "creasePattern"))
    {
        ignored_field_names.push("frame_classes".to_owned());
    }
    if raw
        .frame_attributes
        .as_ref()
        .is_some_and(|attributes| attributes.iter().any(|attribute| attribute != "2D"))
    {
        ignored_field_names.push("frame_attributes".to_owned());
    }
    ignored_field_names.sort();
    ignored_field_names.dedup();

    let title = raw.file_title.or(raw.frame_title);
    if let Some(title) = &title {
        let actual = title.chars().count();
        if actual > MAX_FOLD_TITLE_CHARS {
            return Err(FoldImportError::TitleTooLong {
                actual,
                maximum: MAX_FOLD_TITLE_CHARS,
            });
        }
    }
    let frame_unit = parse_frame_unit(raw.frame_unit)?;

    let mut position_indices = HashMap::with_capacity(raw.vertices_coords.len());
    let mut vertices = Vec::with_capacity(raw.vertices_coords.len());
    for (index, coordinates) in raw.vertices_coords.into_iter().enumerate() {
        if !matches!(coordinates.dimensions, 2 | 3) {
            return Err(FoldImportError::UnsupportedCoordinateDimension {
                vertex_index: index,
                dimensions: coordinates.dimensions,
            });
        }
        if coordinates.values[..coordinates.dimensions]
            .iter()
            .any(|coordinate| !coordinate.is_finite())
        {
            return Err(FoldImportError::NonFiniteCoordinate {
                vertex_index: index,
            });
        }
        if coordinates.dimensions == 3 && coordinates.values[2] != 0.0 {
            return Err(FoldImportError::NonPlanarCoordinate {
                vertex_index: index,
                z: coordinates.values[2],
            });
        }
        let x = coordinates.values[0];
        let y = coordinates.values[1];
        let key = (canonical_f64_bits(x), canonical_f64_bits(y));
        if let Some(first_index) = position_indices.insert(key, index) {
            return Err(FoldImportError::DuplicateVertex {
                first_index,
                duplicate_index: index,
            });
        }
        vertices.push(FoldPreviewVertex {
            index,
            position: Point2::new(x, y),
        });
    }

    let mut edge_indices = HashMap::with_capacity(raw.edges_vertices.len());
    let mut counts = FoldAssignmentCounts::default();
    let mut edges = Vec::with_capacity(raw.edges_vertices.len());
    for (index, (endpoints, assignment)) in raw
        .edges_vertices
        .into_iter()
        .zip(raw.edges_assignment)
        .enumerate()
    {
        for endpoint in endpoints {
            if endpoint >= vertices.len() {
                return Err(FoldImportError::VertexIndexOutOfBounds {
                    edge_index: index,
                    vertex_index: endpoint,
                    vertex_count: vertices.len(),
                });
            }
        }
        if endpoints[0] == endpoints[1] {
            return Err(FoldImportError::DegenerateEdge { edge_index: index });
        }
        let start = vertices[endpoints[0]].position;
        let end = vertices[endpoints[1]].position;
        if !(end.x - start.x).is_finite() || !(end.y - start.y).is_finite() {
            return Err(FoldImportError::EdgeSpanNotRepresentable { edge_index: index });
        }
        let key = if endpoints[0] < endpoints[1] {
            (endpoints[0], endpoints[1])
        } else {
            (endpoints[1], endpoints[0])
        };
        if let Some(first_index) = edge_indices.insert(key, index) {
            return Err(FoldImportError::DuplicateEdge {
                first_index,
                duplicate_index: index,
            });
        }
        counts.add(assignment);
        edges.push(FoldPreviewEdge {
            index,
            vertices: endpoints,
            assignment,
        });
    }
    validate_intersection_candidate_limit(&edges, &vertices, limits.max_intersection_candidates)?;
    if raw.file_spec.is_some_and(|file_spec| file_spec < 1.2) {
        for assignment in [FoldEdgeAssignment::Cut, FoldEdgeAssignment::Join] {
            if counts.count(assignment) > 0 {
                return Err(FoldImportError::AssignmentRequiresSpec12 { assignment });
            }
        }
    }

    let boundary_vertex_indices = boundary_cycle(&edges, &vertices, limits)?;
    let mut warnings = Vec::new();
    if raw.file_spec.is_none() {
        warnings.push(FoldPreviewWarning::MissingFileSpec);
    }
    if frame_unit.millimetres_per_unit().is_none() {
        warnings.push(FoldPreviewWarning::UnitNeedsScaleSelection);
    }
    if !ignored_field_names.is_empty() {
        warnings.push(FoldPreviewWarning::IgnoredFields {
            names: ignored_field_names,
        });
    }

    Ok(FoldPreview {
        file_spec: raw.file_spec,
        title,
        frame_unit,
        vertices,
        edges,
        boundary_vertex_indices,
        assignment_counts: counts,
        warnings,
    })
}

fn parse_frame_unit(unit: Option<String>) -> Result<FoldFrameUnit, FoldImportError> {
    let Some(unit) = unit else {
        return Ok(FoldFrameUnit::Unspecified);
    };
    Ok(match unit.as_str() {
        "unit" => FoldFrameUnit::Unitless,
        "in" => FoldFrameUnit::Inch,
        "pt" => FoldFrameUnit::Point,
        "m" => FoldFrameUnit::Metre,
        "cm" => FoldFrameUnit::Centimetre,
        "mm" => FoldFrameUnit::Millimetre,
        "um" => FoldFrameUnit::Micrometre,
        "nm" => FoldFrameUnit::Nanometre,
        _ => {
            if unit.is_empty()
                || unit.chars().count() > MAX_FOLD_UNIT_CHARS
                || unit.chars().any(char::is_control)
            {
                return Err(FoldImportError::InvalidFrameUnit);
            }
            FoldFrameUnit::Custom(unit)
        }
    })
}

fn validate_intersection_candidate_limit(
    edges: &[FoldPreviewEdge],
    vertices: &[FoldPreviewVertex],
    maximum: usize,
) -> Result<(), FoldImportError> {
    let mut bounds = edges
        .iter()
        .map(|edge| {
            let start = vertices[edge.vertices[0]].position;
            let end = vertices[edge.vertices[1]].position;
            IntersectionCandidateBounds::new(edge.index, start, end)
        })
        .collect::<Vec<_>>();
    if intersection_candidate_limit_exceeded(&mut bounds, maximum) {
        return Err(FoldImportError::TooManyIntersectionCandidates { maximum });
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct IntersectionCandidateBounds {
    edge_index: usize,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl IntersectionCandidateBounds {
    fn new(edge_index: usize, start: Point2, end: Point2) -> Self {
        Self {
            edge_index,
            min_x: start.x.min(end.x),
            max_x: start.x.max(end.x),
            min_y: start.y.min(end.y),
            max_y: start.y.max(end.y),
        }
    }
}

fn intersection_candidate_limit_exceeded(
    bounds: &mut [IntersectionCandidateBounds],
    maximum: usize,
) -> bool {
    bounds.sort_unstable_by(|left, right| {
        left.min_x
            .total_cmp(&right.min_x)
            .then_with(|| left.edge_index.cmp(&right.edge_index))
    });

    let mut candidates = 0_usize;
    for (position, left) in bounds.iter().copied().enumerate() {
        for right in bounds.iter().copied().skip(position + 1) {
            if right.min_x > left.max_x {
                break;
            }
            if left.min_y > right.max_y || right.min_y > left.max_y {
                continue;
            }
            candidates = candidates.saturating_add(1);
            if candidates > maximum {
                return true;
            }
        }
    }
    false
}

fn boundary_cycle(
    edges: &[FoldPreviewEdge],
    vertices: &[FoldPreviewVertex],
    limits: FoldImportLimits,
) -> Result<Vec<usize>, FoldImportError> {
    let boundary: Vec<_> = edges
        .iter()
        .filter(|edge| edge.assignment == FoldEdgeAssignment::Boundary)
        .collect();
    if boundary.len() > limits.max_boundary_edges {
        return Err(FoldImportError::TooManyBoundaryEdges {
            actual: boundary.len(),
            maximum: limits.max_boundary_edges,
        });
    }
    if boundary.len() < 3 {
        return Err(FoldImportError::BoundaryTooSmall {
            actual: boundary.len(),
        });
    }
    let mut adjacency = vec![Vec::new(); vertices.len()];
    for edge in &boundary {
        adjacency[edge.vertices[0]].push(edge.vertices[1]);
        adjacency[edge.vertices[1]].push(edge.vertices[0]);
    }
    for (vertex_index, neighbours) in adjacency.iter().enumerate() {
        if !neighbours.is_empty() && neighbours.len() != 2 {
            return Err(FoldImportError::BoundaryDegree {
                vertex_index,
                degree: neighbours.len(),
            });
        }
    }
    let start = adjacency
        .iter()
        .position(|neighbours| !neighbours.is_empty())
        .expect("at least three boundary edges");
    adjacency[start].sort_unstable();
    let mut cycle = Vec::with_capacity(boundary.len());
    let mut seen = HashSet::with_capacity(boundary.len());
    let mut previous = None;
    let mut current = start;
    loop {
        if !seen.insert(current) {
            return if current == start && cycle.len() == boundary.len() {
                validate_boundary_geometry(&cycle, vertices)?;
                Ok(cycle)
            } else {
                Err(FoldImportError::BoundaryDisconnected)
            };
        }
        cycle.push(current);
        let neighbours = &adjacency[current];
        let next = if Some(neighbours[0]) == previous {
            neighbours[1]
        } else {
            neighbours[0]
        };
        previous = Some(current);
        current = next;
    }
}

fn validate_boundary_geometry(
    cycle: &[usize],
    vertices: &[FoldPreviewVertex],
) -> Result<(), FoldImportError> {
    let positions: Vec<_> = cycle
        .iter()
        .map(|index| vertices[*index].position)
        .collect();
    match exact_polygon_orientation(&positions)
        .map_err(|_| FoldImportError::BoundaryGeometryUnrepresentable)?
    {
        Orientation::Collinear => return Err(FoldImportError::BoundaryZeroArea),
        Orientation::Clockwise | Orientation::CounterClockwise => {}
    }

    for first_edge in 0..cycle.len() {
        let first_start = positions[first_edge];
        let first_end = positions[(first_edge + 1) % cycle.len()];
        for second_edge in (first_edge + 1)..cycle.len() {
            let second_start = positions[second_edge];
            let second_end = positions[(second_edge + 1) % cycle.len()];
            if first_start.x.min(first_end.x) > second_start.x.max(second_end.x)
                || second_start.x.min(second_end.x) > first_start.x.max(first_end.x)
                || first_start.y.min(first_end.y) > second_start.y.max(second_end.y)
                || second_start.y.min(second_end.y) > first_start.y.max(first_end.y)
            {
                continue;
            }
            let adjacent = first_edge.abs_diff(second_edge) == 1
                || (first_edge == 0 && second_edge == cycle.len() - 1);
            let intersection =
                segment_intersection(first_start, first_end, second_start, second_end)
                    .map_err(|_| FoldImportError::BoundaryGeometryUnrepresentable)?;
            let valid_adjacent_point = adjacent
                && matches!(
                    intersection,
                    SegmentIntersection::Point(point)
                        if point == first_start
                            || point == first_end
                );
            if !matches!(intersection, SegmentIntersection::None) && !valid_adjacent_point {
                return Err(FoldImportError::BoundarySelfIntersection {
                    first_edge,
                    second_edge,
                });
            }
        }
    }
    Ok(())
}

fn convert_preview(
    preview: &FoldPreview,
    options: &FoldConversionOptions,
) -> Result<FoldCreasePatternConversion, FoldConversionError> {
    let scale = options.millimetres_per_unit;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(FoldConversionError::InvalidMillimetresPerUnit);
    }
    for assignment in [
        FoldEdgeAssignment::Boundary,
        FoldEdgeAssignment::Mountain,
        FoldEdgeAssignment::Valley,
        FoldEdgeAssignment::Flat,
        FoldEdgeAssignment::Unassigned,
        FoldEdgeAssignment::Cut,
        FoldEdgeAssignment::Join,
    ] {
        if preview.assignment_counts.count(assignment) > 0 {
            let target = options
                .assignment_mapping
                .target(assignment)
                .ok_or(FoldConversionError::MappingNotSpecified { assignment })?;
            validate_mapping(assignment, target)?;
        }
    }

    let vertex_ids: Vec<_> = preview.vertices.iter().map(|_| VertexId::new()).collect();
    let mut scaled_positions = HashMap::with_capacity(preview.vertices.len());
    let mut vertices = Vec::with_capacity(preview.vertices.len());
    for vertex in &preview.vertices {
        let position = Point2::new(vertex.position.x * scale, vertex.position.y * scale);
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(FoldConversionError::ScaledCoordinateNotFinite {
                vertex_index: vertex.index,
            });
        }
        let key = (
            canonical_f64_bits(position.x),
            canonical_f64_bits(position.y),
        );
        if let Some(first_index) = scaled_positions.insert(key, vertex.index) {
            return Err(FoldConversionError::ScaledVertexCollision {
                first_index,
                duplicate_index: vertex.index,
            });
        }
        vertices.push(Vertex {
            id: vertex_ids[vertex.index],
            position,
        });
    }

    let mut edge_ids = Vec::with_capacity(preview.edges.len());
    let mut edges = Vec::with_capacity(preview.edges.len());
    let mut scaled_candidate_bounds = Vec::with_capacity(preview.edges.len());
    for edge in &preview.edges {
        let target = options.assignment_mapping.target(edge.assignment).ok_or(
            FoldConversionError::MappingNotSpecified {
                assignment: edge.assignment,
            },
        )?;
        match target {
            FoldAssignmentTarget::Ignore => edge_ids.push(None),
            FoldAssignmentTarget::ImportAs { edge_kind } => {
                let id = EdgeId::new();
                edge_ids.push(Some(id));
                scaled_candidate_bounds.push(IntersectionCandidateBounds::new(
                    edge.index,
                    vertices[edge.vertices[0]].position,
                    vertices[edge.vertices[1]].position,
                ));
                edges.push(Edge {
                    id,
                    start: vertex_ids[edge.vertices[0]],
                    end: vertex_ids[edge.vertices[1]],
                    kind: edge_kind,
                });
            }
        }
    }
    if intersection_candidate_limit_exceeded(
        &mut scaled_candidate_bounds,
        MAX_FOLD_INTERSECTION_CANDIDATES,
    ) {
        return Err(FoldConversionError::TooManyIntersectionCandidates {
            maximum: MAX_FOLD_INTERSECTION_CANDIDATES,
        });
    }
    let boundary_vertices = preview
        .boundary_vertex_indices
        .iter()
        .map(|index| vertex_ids[*index])
        .collect();
    Ok(FoldCreasePatternConversion {
        crease_pattern: CreasePattern { vertices, edges },
        vertex_ids,
        edge_ids,
        boundary_vertices,
    })
}

fn validate_mapping(
    assignment: FoldEdgeAssignment,
    target: FoldAssignmentTarget,
) -> Result<(), FoldConversionError> {
    let valid = matches!(
        (assignment, target),
        (
            FoldEdgeAssignment::Boundary,
            FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Boundary
            }
        ) | (
            FoldEdgeAssignment::Mountain,
            FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Mountain
            }
        ) | (
            FoldEdgeAssignment::Valley,
            FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Valley
            }
        ) | (
            FoldEdgeAssignment::Flat | FoldEdgeAssignment::Join,
            FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Auxiliary
            } | FoldAssignmentTarget::Ignore
        ) | (
            FoldEdgeAssignment::Unassigned,
            FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Auxiliary
            } | FoldAssignmentTarget::Ignore
        ) | (
            FoldEdgeAssignment::Cut,
            FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Cut
            } | FoldAssignmentTarget::Ignore
        )
    );
    if valid {
        Ok(())
    } else {
        Err(FoldConversionError::InvalidMapping { assignment, target })
    }
}

fn canonical_f64_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square_json(extra: &str) -> Vec<u8> {
        format!(
            r#"{{
                "file_spec": 1.2,
                "file_title": "Square",
                "frame_unit": "cm",
                "vertices_coords": [[0,0],[10,0],[10,10],[0,10]],
                "edges_vertices": [[0,1],[1,2],[2,3],[3,0],[0,2]],
                "edges_assignment": ["B","B","B","B","M"]
                {extra}
            }}"#
        )
        .into_bytes()
    }

    fn complete_mapping() -> FoldAssignmentMapping {
        FoldAssignmentMapping {
            boundary: Some(FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Boundary,
            }),
            mountain: Some(FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Mountain,
            }),
            valley: Some(FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Valley,
            }),
            flat: Some(FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Auxiliary,
            }),
            unassigned: Some(FoldAssignmentTarget::Ignore),
            cut: Some(FoldAssignmentTarget::ImportAs {
                edge_kind: EdgeKind::Cut,
            }),
            join: Some(FoldAssignmentTarget::Ignore),
        }
    }

    fn options() -> FoldConversionOptions {
        FoldConversionOptions {
            assignment_mapping: complete_mapping(),
            millimetres_per_unit: 10.0,
        }
    }

    #[test]
    fn parses_supported_root_frame_into_indexed_preview() {
        let preview =
            read_fold_preview(&square_json(r#","file_creator":"test""#)).expect("valid preview");

        assert_eq!(preview.file_spec(), Some(1.2));
        assert_eq!(preview.title(), Some("Square"));
        assert_eq!(preview.frame_unit(), &FoldFrameUnit::Centimetre);
        assert_eq!(preview.recommended_millimetres_per_unit(), Some(10.0));
        assert_eq!(preview.vertices().len(), 4);
        assert_eq!(preview.vertices()[2].index, 2);
        assert_eq!(preview.vertices()[2].position, Point2::new(10.0, 10.0));
        assert_eq!(preview.edges()[4].index, 4);
        assert_eq!(preview.edges()[4].vertices, [0, 2]);
        assert_eq!(preview.edges()[4].assignment, FoldEdgeAssignment::Mountain);
        assert_eq!(preview.boundary_vertex_indices(), &[0, 1, 2, 3]);
        assert_eq!(preview.assignment_counts().boundary, 4);
        assert_eq!(preview.assignment_counts().mountain, 1);
        assert!(matches!(
            preview.warnings(),
            [FoldPreviewWarning::IgnoredFields { names }] if names == &["file_creator"]
        ));
    }

    #[test]
    fn converts_only_after_explicit_mapping_and_scales_to_millimetres() {
        let preview = read_fold_preview(&square_json("")).expect("valid preview");
        let converted = preview.convert(&options()).expect("convert");

        assert_eq!(converted.crease_pattern().vertices.len(), 4);
        assert_eq!(converted.crease_pattern().edges.len(), 5);
        assert_eq!(
            converted.crease_pattern().vertices[2].position,
            Point2::new(100.0, 100.0)
        );
        assert_eq!(converted.crease_pattern().edges[4].kind, EdgeKind::Mountain);
        assert_eq!(converted.vertex_ids().len(), 4);
        assert_eq!(converted.edge_ids().len(), 5);
        assert_eq!(converted.boundary_vertices().len(), 4);
        for (source_index, id) in converted.vertex_ids().iter().enumerate() {
            assert_eq!(converted.crease_pattern().vertices[source_index].id, *id);
        }
        assert_eq!(converted.boundary_vertices(), &converted.vertex_ids()[0..4]);
    }

    #[test]
    fn ignore_removes_an_edge_but_preserves_source_index_mapping() {
        let bytes = String::from_utf8(square_json(""))
            .expect("fixture UTF-8")
            .replace(r#""B","M"]"#, r#""B","F"]"#);
        let preview = read_fold_preview(bytes.as_bytes()).expect("valid preview");
        let mut options = options();
        options.assignment_mapping.flat = Some(FoldAssignmentTarget::Ignore);

        let converted = preview.convert(&options).expect("convert");

        assert_eq!(converted.crease_pattern().edges.len(), 4);
        assert_eq!(converted.edge_ids()[4], None);
        assert!(converted.edge_ids()[0..4].iter().all(Option::is_some));
    }

    #[test]
    fn rejects_missing_and_semantically_invalid_mapping() {
        let preview = read_fold_preview(&square_json("")).expect("valid preview");
        let mut missing = options();
        missing.assignment_mapping.mountain = None;
        assert!(matches!(
            preview.convert(&missing),
            Err(FoldConversionError::MappingNotSpecified {
                assignment: FoldEdgeAssignment::Mountain
            })
        ));

        let mut invalid = options();
        invalid.assignment_mapping.boundary = Some(FoldAssignmentTarget::Ignore);
        assert!(matches!(
            preview.convert(&invalid),
            Err(FoldConversionError::InvalidMapping {
                assignment: FoldEdgeAssignment::Boundary,
                target: FoldAssignmentTarget::Ignore
            })
        ));
    }

    #[test]
    fn assignment_policy_matches_supported_domain_semantics() {
        let allowed = [
            (FoldEdgeAssignment::Boundary, EdgeKind::Boundary),
            (FoldEdgeAssignment::Mountain, EdgeKind::Mountain),
            (FoldEdgeAssignment::Valley, EdgeKind::Valley),
            (FoldEdgeAssignment::Flat, EdgeKind::Auxiliary),
            (FoldEdgeAssignment::Unassigned, EdgeKind::Mountain),
            (FoldEdgeAssignment::Unassigned, EdgeKind::Valley),
            (FoldEdgeAssignment::Unassigned, EdgeKind::Auxiliary),
            (FoldEdgeAssignment::Cut, EdgeKind::Cut),
            (FoldEdgeAssignment::Join, EdgeKind::Auxiliary),
        ];
        for (assignment, edge_kind) in allowed {
            assert_eq!(
                validate_mapping(assignment, FoldAssignmentTarget::ImportAs { edge_kind }),
                Ok(())
            );
        }
        for assignment in [
            FoldEdgeAssignment::Flat,
            FoldEdgeAssignment::Unassigned,
            FoldEdgeAssignment::Cut,
            FoldEdgeAssignment::Join,
        ] {
            assert_eq!(
                validate_mapping(assignment, FoldAssignmentTarget::Ignore),
                Ok(())
            );
        }
        for assignment in [
            FoldEdgeAssignment::Boundary,
            FoldEdgeAssignment::Mountain,
            FoldEdgeAssignment::Valley,
        ] {
            assert!(validate_mapping(assignment, FoldAssignmentTarget::Ignore).is_err());
        }
    }

    #[test]
    fn standard_units_have_exact_documented_millimetre_scales() {
        assert_eq!(FoldFrameUnit::Inch.millimetres_per_unit(), Some(25.4));
        assert_eq!(
            FoldFrameUnit::Point.millimetres_per_unit(),
            Some(25.4 / 72.0)
        );
        assert_eq!(FoldFrameUnit::Metre.millimetres_per_unit(), Some(1_000.0));
        assert_eq!(FoldFrameUnit::Centimetre.millimetres_per_unit(), Some(10.0));
        assert_eq!(FoldFrameUnit::Millimetre.millimetres_per_unit(), Some(1.0));
        assert_eq!(
            FoldFrameUnit::Micrometre.millimetres_per_unit(),
            Some(0.001)
        );
        assert_eq!(
            FoldFrameUnit::Nanometre.millimetres_per_unit(),
            Some(0.000_001)
        );
        assert_eq!(FoldFrameUnit::Unitless.millimetres_per_unit(), None);
        assert_eq!(FoldFrameUnit::Unspecified.millimetres_per_unit(), None);
    }

    #[test]
    fn unitless_custom_and_missing_metadata_produce_actionable_warnings() {
        let bytes = br#"{
            "frame_unit":"fold",
            "vertices_coords":[[0,0],[1,0],[0,1]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        let preview = read_fold_preview(bytes).expect("custom unit preview");
        assert_eq!(
            preview.frame_unit(),
            &FoldFrameUnit::Custom("fold".to_owned())
        );
        assert!(
            preview
                .warnings()
                .contains(&FoldPreviewWarning::MissingFileSpec)
        );
        assert!(
            preview
                .warnings()
                .contains(&FoldPreviewWarning::UnitNeedsScaleSelection)
        );
    }

    #[test]
    fn rejects_file_collection_and_parallel_array_shape_errors() {
        let invalid_cases: &[&[u8]] = &[
            br#"[]"#,
            br#"{"edges_vertices":[],"edges_assignment":[]}"#,
            br#"{"vertices_coords":null,"edges_vertices":[],"edges_assignment":[]}"#,
            br#"{"vertices_coords":[[0,"x"]],"edges_vertices":[],"edges_assignment":[]}"#,
            br#"{"vertices_coords":[],"edges_vertices":[[0]],"edges_assignment":["B"]}"#,
            br#"{"vertices_coords":[],"edges_vertices":[],"edges_assignment":["b"]}"#,
        ];
        for bytes in invalid_cases {
            assert!(matches!(
                read_fold_preview(bytes),
                Err(FoldImportError::InvalidJson(_))
            ));
        }
    }

    #[test]
    fn rejects_null_in_supported_optional_metadata_fields() {
        for field in [
            "file_spec",
            "file_title",
            "frame_title",
            "frame_classes",
            "frame_attributes",
            "frame_unit",
        ] {
            let bytes = format!(
                r#"{{
                    "{field}":null,
                    "vertices_coords":[[0,0],[1,0],[0,1]],
                    "edges_vertices":[[0,1],[1,2],[2,0]],
                    "edges_assignment":["B","B","B"]
                }}"#
            );
            assert!(
                matches!(
                    read_fold_preview(bytes.as_bytes()),
                    Err(FoldImportError::InvalidJson(_))
                ),
                "{field} null must not be treated as omission"
            );
        }
    }

    #[test]
    fn rejects_non_finite_or_duplicate_vertices_and_bad_edge_spans() {
        let non_finite = br#"{
            "vertices_coords":[[0,0],[1e400,0],[0,1]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(non_finite),
            Err(FoldImportError::InvalidJson(_)) | Err(FoldImportError::NonFiniteCoordinate { .. })
        ));

        let duplicate = br#"{
            "vertices_coords":[[0,0],[-0.0,0],[0,1]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(duplicate),
            Err(FoldImportError::DuplicateVertex {
                first_index: 0,
                duplicate_index: 1
            })
        ));

        let overflow = br#"{
            "vertices_coords":[[-1.7e308,0],[1.7e308,0],[0,1]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(overflow),
            Err(FoldImportError::EdgeSpanNotRepresentable { edge_index: 0 })
        ));
    }

    #[test]
    fn rejects_bad_indices_degenerate_edges_and_undirected_duplicates() {
        let bad_index = br#"{
            "vertices_coords":[[0,0],[1,0],[0,1]],
            "edges_vertices":[[0,3],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(bad_index),
            Err(FoldImportError::VertexIndexOutOfBounds {
                edge_index: 0,
                vertex_index: 3,
                vertex_count: 3
            })
        ));

        let degenerate = br#"{
            "vertices_coords":[[0,0],[1,0],[0,1]],
            "edges_vertices":[[0,0],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(degenerate),
            Err(FoldImportError::DegenerateEdge { edge_index: 0 })
        ));

        let duplicate = br#"{
            "vertices_coords":[[0,0],[1,0],[0,1]],
            "edges_vertices":[[0,1],[1,0],[1,2],[2,0]],
            "edges_assignment":["B","M","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(duplicate),
            Err(FoldImportError::DuplicateEdge {
                first_index: 0,
                duplicate_index: 1
            })
        ));
    }

    #[test]
    fn rejects_assignment_length_mismatch_before_boundary_analysis() {
        let bytes = br#"{
            "vertices_coords":[[0,0],[1,0],[0,1]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(bytes),
            Err(FoldImportError::AssignmentCountMismatch {
                edges: 3,
                assignments: 2
            })
        ));
    }

    #[test]
    fn rejects_branching_disconnected_and_self_intersecting_boundaries() {
        let branching = br#"{
            "vertices_coords":[[0,0],[2,0],[2,2],[0,2],[1,1]],
            "edges_vertices":[[0,1],[1,2],[2,3],[3,0],[0,4],[4,2]],
            "edges_assignment":["B","B","B","B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(branching),
            Err(FoldImportError::BoundaryDegree {
                vertex_index: 0,
                degree: 3
            })
        ));

        let disconnected = br#"{
            "vertices_coords":[[0,0],[1,0],[0,1],[3,0],[4,0],[3,1]],
            "edges_vertices":[[0,1],[1,2],[2,0],[3,4],[4,5],[5,3]],
            "edges_assignment":["B","B","B","B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(disconnected),
            Err(FoldImportError::BoundaryDisconnected)
        ));

        let crossing = br#"{
            "vertices_coords":[[0,0],[3,3],[0,3],[2,0]],
            "edges_vertices":[[0,1],[1,2],[2,3],[3,0]],
            "edges_assignment":["B","B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(crossing),
            Err(FoldImportError::BoundarySelfIntersection { .. })
        ));
    }

    #[test]
    fn limits_file_vertices_edges_and_boundary_separately() {
        let bytes = square_json("");
        let limits = FoldImportLimits {
            max_file_bytes: bytes.len() - 1,
            ..FoldImportLimits::default()
        };
        assert!(matches!(
            read_fold_preview_with_limits(&bytes, limits),
            Err(FoldImportError::FileTooLarge { .. })
        ));

        let limits = FoldImportLimits {
            max_vertices: 3,
            ..FoldImportLimits::default()
        };
        assert!(matches!(
            read_fold_preview_with_limits(&bytes, limits),
            Err(FoldImportError::TooManyVertices {
                actual: 4,
                maximum: 3
            })
        ));

        let limits = FoldImportLimits {
            max_edges: 4,
            ..FoldImportLimits::default()
        };
        assert!(matches!(
            read_fold_preview_with_limits(&bytes, limits),
            Err(FoldImportError::TooManyEdges {
                actual: 5,
                maximum: 4
            })
        ));

        let limits = FoldImportLimits {
            max_boundary_edges: 3,
            ..FoldImportLimits::default()
        };
        assert!(matches!(
            read_fold_preview_with_limits(&bytes, limits),
            Err(FoldImportError::TooManyBoundaryEdges {
                actual: 4,
                maximum: 3
            })
        ));

        let limits = FoldImportLimits {
            max_intersection_candidates: 0,
            ..FoldImportLimits::default()
        };
        assert!(matches!(
            read_fold_preview_with_limits(&bytes, limits),
            Err(FoldImportError::TooManyIntersectionCandidates { maximum: 0 })
        ));
    }

    #[test]
    fn conversion_rechecks_intersection_work_after_scale_rounding() {
        let interior_edge_count = 1_415;
        let mut vertices = vec![
            [-1.0, -1.0],
            [102_000.0, -1.0],
            [102_000.0, 2.0],
            [-1.0, 2.0],
        ];
        let mut edges = vec![[0_usize, 1_usize], [1, 2], [2, 3], [3, 0]];
        let mut assignments = vec!["B"; 4];
        for index in 0..interior_edge_count {
            let y = f64::from_bits(1.0_f64.to_bits() + index as u64);
            let start = vertices.len();
            vertices.push([index as f64, y]);
            vertices.push([100_000.0 + index as f64, y]);
            edges.push([start, start + 1]);
            assignments.push("F");
        }
        let bytes = serde_json::to_vec(&serde_json::json!({
            "file_spec": 1.2,
            "vertices_coords": vertices,
            "edges_vertices": edges,
            "edges_assignment": assignments,
        }))
        .expect("serialize scaling work fixture");
        let preview = read_fold_preview(&bytes).expect("raw geometry remains below the work cap");
        let mut options = options();
        options.millimetres_per_unit = f64::from_bits(1);

        assert_eq!(
            preview.convert(&options),
            Err(FoldConversionError::TooManyIntersectionCandidates {
                maximum: MAX_FOLD_INTERSECTION_CANDIDATES
            })
        );
    }

    #[test]
    fn rejects_invalid_scale_and_scaled_coordinate_overflow() {
        let preview = read_fold_preview(&square_json("")).expect("valid preview");
        for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut options = options();
            options.millimetres_per_unit = scale;
            assert_eq!(
                preview.convert(&options),
                Err(FoldConversionError::InvalidMillimetresPerUnit)
            );
        }

        let mut options = options();
        options.millimetres_per_unit = f64::MAX;
        assert!(matches!(
            preview.convert(&options),
            Err(FoldConversionError::ScaledCoordinateNotFinite { .. })
        ));
    }

    #[test]
    fn accepts_two_dimensions_and_explicit_zero_z_but_rejects_other_geometry() {
        let explicit_zero_z = br#"{
            "file_spec":1.2,
            "vertices_coords":[[0,0,0],[1,0,-0.0],[0,1,0]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        let preview = read_fold_preview(explicit_zero_z).expect("explicit zero z is 2D");
        assert_eq!(preview.vertices()[1].position, Point2::new(1.0, 0.0));

        let nonzero_z = br#"{
            "file_spec":1.2,
            "vertices_coords":[[0,0,0],[1,0,0.1],[0,1,0]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(nonzero_z),
            Err(FoldImportError::NonPlanarCoordinate {
                vertex_index: 1,
                z: 0.1
            })
        ));

        let four_dimensions = br#"{
            "file_spec":1.2,
            "vertices_coords":[[0,0,0,0],[1,0],[0,1]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(four_dimensions),
            Err(FoldImportError::UnsupportedCoordinateDimension {
                vertex_index: 0,
                dimensions: 4
            })
        ));
    }

    #[test]
    fn rejects_folded_form_and_explicit_3d_frames() {
        let folded = String::from_utf8(square_json(r#","frame_classes":["foldedForm"]"#))
            .expect("fixture UTF-8");
        assert!(matches!(
            read_fold_preview(folded.as_bytes()),
            Err(FoldImportError::UnsupportedFrameClass { class }) if class == "foldedForm"
        ));

        let three_d =
            String::from_utf8(square_json(r#","frame_attributes":["3D"]"#)).expect("fixture UTF-8");
        assert!(matches!(
            read_fold_preview(three_d.as_bytes()),
            Err(FoldImportError::UnsupportedFrameAttribute { attribute }) if attribute == "3D"
        ));
    }

    #[test]
    fn warns_when_supported_metadata_is_present_but_not_persisted() {
        let bytes = square_json(
            r#","frame_title":"Frame title","frame_classes":["creasePattern","cuts"],"frame_attributes":["2D","cuts"]"#,
        );
        let preview = read_fold_preview(&bytes).expect("metadata remains previewable");

        assert!(matches!(
            preview.warnings(),
            [FoldPreviewWarning::IgnoredFields { names }]
                if names == &["frame_attributes", "frame_classes", "frame_title"]
        ));
    }

    #[test]
    fn accepts_only_known_spec_versions_and_gates_12_assignments() {
        for supported in ["1", "1.0", "1.1", "1.2"] {
            let bytes = String::from_utf8(square_json(""))
                .expect("fixture UTF-8")
                .replace("\"file_spec\": 1.2", &format!("\"file_spec\": {supported}"));
            read_fold_preview(bytes.as_bytes()).expect("supported version");
        }

        for unsupported in ["0.9", "1.3", "2.0"] {
            let bytes = String::from_utf8(square_json(""))
                .expect("fixture UTF-8")
                .replace(
                    "\"file_spec\": 1.2",
                    &format!("\"file_spec\": {unsupported}"),
                );
            assert!(matches!(
                read_fold_preview(bytes.as_bytes()),
                Err(FoldImportError::UnsupportedFileSpec { .. })
            ));
        }

        for assignment in ["C", "J"] {
            let bytes = format!(
                r#"{{
                    "file_spec":1.1,
                    "vertices_coords":[[0,0],[1,0],[1,1],[0,1]],
                    "edges_vertices":[[0,1],[1,2],[2,3],[3,0],[0,2]],
                    "edges_assignment":["B","B","B","B","{assignment}"]
                }}"#
            );
            assert!(matches!(
                read_fold_preview(bytes.as_bytes()),
                Err(FoldImportError::AssignmentRequiresSpec12 { .. })
            ));
        }
    }

    #[test]
    fn duplicate_supported_json_keys_are_rejected() {
        let bytes = br#"{
            "file_spec":1.2,
            "vertices_coords":[[0,0],[1,0],[0,1]],
            "vertices_coords":[[0,0],[2,0],[0,2]],
            "edges_vertices":[[0,1],[1,2],[2,0]],
            "edges_assignment":["B","B","B"]
        }"#;
        assert!(matches!(
            read_fold_preview(bytes),
            Err(FoldImportError::InvalidJson(_))
        ));
    }
}
