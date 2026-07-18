//! Strict, bounded import of a static, straight-line SVG subset.
//!
//! The adapter intentionally does not render SVG. It reads a small interchange
//! subset, exposes source style groups and closed-contour boundary candidates,
//! and converts only after the caller supplies an explicit physical scale and
//! a complete mapping. Unsupported visual content is reported and omitted
//! instead of being approximated.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    str::FromStr,
};

use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, RgbaColor, Vertex, VertexId};
use ori_geometry::{
    Orientation, SegmentIntersection, exact_polygon_orientation, segment_intersection,
};
use quick_xml::{
    XmlVersion,
    events::{BytesDecl, BytesStart, Event},
    name::{Namespace, ResolveResult},
    reader::NsReader,
};
use serde::{Deserialize, Serialize};
use svgtypes::{
    Align, AspectRatio, Length, LengthListParser, LengthUnit, NumberListParser, Paint, PathParser,
    PathSegment, TransformListParser, TransformListToken, ViewBox,
};
use thiserror::Error;

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const MAX_STYLE_TEXT_BYTES: usize = 256 * 1024;
const MAX_DASH_ITEMS: usize = 32;
const MAX_CLASS_TOKENS: usize = 32;
const MAX_HINT_CHARS: usize = 120;
const MAX_STYLE_VALUE_CHARS: usize = 120;
const MAX_CSS_SELECTOR_CHARS: usize = 120;
const MAX_TITLE_CHARS: usize = 512;
const MAX_BOUNDARY_EDGES: usize = 1_414;
const MAX_TRANSFORM_FUNCTIONS: usize = 50_000;

/// Resource limits applied while parsing and converting SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvgImportLimits {
    pub max_file_bytes: usize,
    pub max_depth: usize,
    pub max_elements: usize,
    pub max_attributes_per_element: usize,
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_final_vertices: usize,
    pub max_final_edges: usize,
    pub max_path_commands: usize,
    pub max_css_rules: usize,
    pub max_css_rule_element_evaluations: usize,
    pub max_style_groups: usize,
    pub max_boundary_candidates: usize,
    pub max_warnings: usize,
    pub max_intersection_candidates: usize,
}

impl Default for SvgImportLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 16 * 1024 * 1024,
            max_depth: 64,
            max_elements: 50_000,
            max_attributes_per_element: 64,
            max_source_vertices: 10_000,
            max_source_edges: 10_000,
            max_final_vertices: 10_000,
            max_final_edges: 10_000,
            max_path_commands: 20_000,
            max_css_rules: 128,
            max_css_rule_element_evaluations: 1_000_000,
            max_style_groups: 64,
            max_boundary_candidates: 64,
            max_warnings: 64,
            max_intersection_candidates: 1_000_000,
        }
    }
}

/// Stable identifier for one source style group within one preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SvgStyleGroupId(pub u16);

/// Stable identifier for one boundary candidate within one preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SvgBoundaryCandidateId(pub u16);

/// Root SVG viewBox values retained for import confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SvgRootViewBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Unit written on one root SVG physical dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SvgRootLengthUnit {
    Mm,
    Cm,
    In,
    Pt,
    Pc,
    Q,
    Px,
    Unitless,
    Em,
    Ex,
    Percent,
}

/// Root physical dimensions after resolving supported units to millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SvgRootPhysicalSize {
    pub width_millimetres: Option<f64>,
    pub height_millimetres: Option<f64>,
    pub width_unit: Option<SvgRootLengthUnit>,
    pub height_unit: Option<SvgRootLengthUnit>,
}

/// Normalized dash information used as a line-type mapping cue.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "lengths", rename_all = "snake_case")]
pub enum SvgDashPattern {
    Solid,
    Dashes(Vec<f64>),
}

/// One source style signature presented for explicit mapping.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SvgStyleGroup {
    pub id: SvgStyleGroupId,
    pub segment_count: usize,
    pub element_count: usize,
    pub representative_id: Option<String>,
    pub stroke: RgbaColor,
    pub stroke_width: f64,
    pub dash_pattern: SvgDashPattern,
    pub classes: Vec<String>,
    pub layer: Option<String>,
    pub semantic: Option<String>,
}

/// Source of a boundary candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SvgBoundaryCandidateKind {
    ViewBox,
    Polygon,
    Polyline,
    Rectangle,
    ClosedPath,
}

/// A closed contour that the caller may choose independently of style mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SvgBoundaryCandidate {
    pub id: SvgBoundaryCandidateId,
    pub kind: SvgBoundaryCandidateKind,
    pub vertex_indices: Vec<usize>,
    pub source_edge_indices: Vec<usize>,
}

/// One source vertex retained for preview rendering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SvgPreviewVertex {
    pub index: usize,
    pub position: Point2,
}

/// One source segment retained for preview rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SvgPreviewEdge {
    pub index: usize,
    pub vertices: [usize; 2],
    pub style_group: SvgStyleGroupId,
}

/// Category for a bounded, aggregated non-fatal import warning.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum SvgWarningKind {
    UnsupportedElement(String),
    UnsupportedAttribute(String),
    UnsupportedStyleProperty(String),
    UnsupportedCssSelector(String),
    UnsupportedPathCommand(char),
    UnsupportedPaint(String),
    UnsupportedLengthUnit(String),
    ExternalReferenceIgnored,
    HiddenGeometryIgnored,
    GeometryWithoutStrokeIgnored,
    FillIgnored,
    MetadataIgnored,
    EmptyGeometryIgnored,
    PhysicalScaleNeedsSelection,
    CssPixelScaleAssumed,
}

/// Aggregated warning. At most `SvgImportLimits::max_warnings` distinct kinds
/// are retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SvgPreviewWarning {
    pub kind: SvgWarningKind,
    pub occurrences: usize,
}

/// Validated SVG source geometry before semantic line mapping.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SvgPreview {
    title: Option<String>,
    root_view_box: Option<SvgRootViewBox>,
    root_physical_size: SvgRootPhysicalSize,
    recommended_millimetres_per_unit: Option<f64>,
    vertices: Vec<SvgPreviewVertex>,
    edges: Vec<SvgPreviewEdge>,
    style_groups: Vec<SvgStyleGroup>,
    boundary_candidates: Vec<SvgBoundaryCandidate>,
    warnings: Vec<SvgPreviewWarning>,
    limits: SvgImportLimits,
}

impl SvgPreview {
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub const fn root_view_box(&self) -> Option<SvgRootViewBox> {
        self.root_view_box
    }

    #[must_use]
    pub const fn root_physical_size(&self) -> SvgRootPhysicalSize {
        self.root_physical_size
    }

    #[must_use]
    pub const fn recommended_millimetres_per_unit(&self) -> Option<f64> {
        self.recommended_millimetres_per_unit
    }

    #[must_use]
    pub fn vertices(&self) -> &[SvgPreviewVertex] {
        &self.vertices
    }

    #[must_use]
    pub fn edges(&self) -> &[SvgPreviewEdge] {
        &self.edges
    }

    #[must_use]
    pub fn style_groups(&self) -> &[SvgStyleGroup] {
        &self.style_groups
    }

    #[must_use]
    pub fn boundary_candidates(&self) -> &[SvgBoundaryCandidate] {
        &self.boundary_candidates
    }

    #[must_use]
    pub fn warnings(&self) -> &[SvgPreviewWarning] {
        &self.warnings
    }

    /// Applies a complete caller-confirmed mapping and physical scale.
    pub fn convert(
        &self,
        options: &SvgConversionOptions,
    ) -> Result<SvgCreasePatternConversion, SvgConversionError> {
        convert_preview(self, options)
    }
}

/// Target semantic kind for one SVG source style group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SvgGroupTarget {
    Boundary,
    Mountain,
    Valley,
    Auxiliary,
    Cut,
    Ignore,
}

impl SvgGroupTarget {
    const fn edge_kind(self) -> Option<EdgeKind> {
        match self {
            Self::Boundary => Some(EdgeKind::Boundary),
            Self::Mountain => Some(EdgeKind::Mountain),
            Self::Valley => Some(EdgeKind::Valley),
            Self::Auxiliary => Some(EdgeKind::Auxiliary),
            Self::Cut => Some(EdgeKind::Cut),
            Self::Ignore => None,
        }
    }
}

/// Mapping decision for one source style group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvgGroupMapping {
    pub group: SvgStyleGroupId,
    pub target: SvgGroupTarget,
}

/// All decisions required to leave SVG preview mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgConversionOptions {
    pub millimetres_per_unit: f64,
    pub group_mappings: Vec<SvgGroupMapping>,
    pub boundary_candidate: Option<SvgBoundaryCandidateId>,
}

/// Converted edge IDs attributable to one source style group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgConvertedGroup {
    pub group: SvgStyleGroupId,
    pub target: SvgGroupTarget,
    pub edge_ids: Vec<EdgeId>,
}

/// Converted pattern and source correspondence.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgCreasePatternConversion {
    crease_pattern: CreasePattern,
    boundary_vertices: Vec<VertexId>,
    groups: Vec<SvgConvertedGroup>,
    has_cuts: bool,
}

impl SvgCreasePatternConversion {
    #[must_use]
    pub const fn crease_pattern(&self) -> &CreasePattern {
        &self.crease_pattern
    }

    #[must_use]
    pub fn boundary_vertices(&self) -> &[VertexId] {
        &self.boundary_vertices
    }

    #[must_use]
    pub fn groups(&self) -> &[SvgConvertedGroup] {
        &self.groups
    }

    #[must_use]
    pub const fn has_cuts(&self) -> bool {
        self.has_cuts
    }

    #[must_use]
    pub fn into_parts(self) -> (CreasePattern, Vec<VertexId>, Vec<SvgConvertedGroup>, bool) {
        (
            self.crease_pattern,
            self.boundary_vertices,
            self.groups,
            self.has_cuts,
        )
    }
}

/// Errors while reading the bounded SVG subset.
#[derive(Debug, Error)]
pub enum SvgImportError {
    #[error("SVG is {actual} bytes; the limit is {maximum} bytes")]
    FileTooLarge { actual: usize, maximum: usize },
    #[error("SVG must be UTF-8")]
    NonUtf8,
    #[error("SVG XML is invalid: {0}")]
    InvalidXml(String),
    #[error("SVG XML declarations may specify only version 1.0 and UTF-8 encoding")]
    UnsupportedXmlDeclaration,
    #[error("SVG DTD and entity declarations are not allowed")]
    DoctypeNotAllowed,
    #[error("SVG root element must be an unprefixed svg element")]
    MissingSvgRoot,
    #[error("SVG root namespace must be {SVG_NAMESPACE}")]
    InvalidSvgNamespace,
    #[error("SVG has content after its root element")]
    TrailingContent,
    #[error("SVG nesting depth {actual} exceeds the limit {maximum}")]
    TooDeep { actual: usize, maximum: usize },
    #[error("SVG has more than {maximum} elements")]
    TooManyElements { maximum: usize },
    #[error("SVG element has more than {maximum} attributes")]
    TooManyAttributes { maximum: usize },
    #[error("SVG has more than {maximum} path commands")]
    TooManyPathCommands { maximum: usize },
    #[error("SVG has more than {maximum} CSS rules")]
    TooManyCssRules { maximum: usize },
    #[error("SVG CSS rule-element evaluation exceeds the limit {maximum}")]
    TooManyCssRuleElementEvaluations { maximum: usize },
    #[error("SVG style property {property} is longer than {maximum} characters")]
    StyleValueTooLong { property: String, maximum: usize },
    #[error("SVG CSS selector is longer than {maximum} characters")]
    CssSelectorTooLong { maximum: usize },
    #[error("SVG has more than {maximum} distinct style groups")]
    TooManyStyleGroups { maximum: usize },
    #[error("SVG has more than {maximum} boundary candidates")]
    TooManyBoundaryCandidates { maximum: usize },
    #[error("SVG boundary candidate validation exceeds {maximum} segment pairs")]
    TooManyBoundaryCandidateIntersections { maximum: usize },
    #[error("SVG produced more than {maximum} source vertices")]
    TooManySourceVertices { maximum: usize },
    #[error("SVG produced more than {maximum} source edges")]
    TooManySourceEdges { maximum: usize },
    #[error("SVG produced more than {maximum} distinct warnings")]
    TooManyWarnings { maximum: usize },
    #[error("SVG title is longer than {maximum} characters")]
    TitleTooLong { maximum: usize },
    #[error("SVG {element} attribute {attribute} is invalid")]
    InvalidAttribute { element: String, attribute: String },
    #[error("SVG {element} contains a non-finite number")]
    NonFiniteNumber { element: String },
    #[error("SVG {element} contains a degenerate straight segment")]
    DegenerateSegment { element: String },
    #[error("SVG transform is invalid or non-invertible")]
    InvalidTransform,
    #[error("SVG transform work exceeds the limit {maximum}")]
    TooManyTransforms { maximum: usize },
    #[error("SVG transformed coordinate is not finite")]
    TransformedCoordinateNotFinite,
    #[error("SVG CSS is invalid")]
    InvalidCss,
    #[error("SVG contains no supported visible straight-line geometry")]
    NoSupportedGeometry,
}

/// Errors while applying mapping, scale and planarization.
#[derive(Debug, Error, PartialEq)]
pub enum SvgConversionError {
    #[error("millimetres per SVG unit must be finite and positive")]
    InvalidMillimetresPerUnit,
    #[error("SVG style group {group:?} has no mapping")]
    MissingGroupMapping { group: SvgStyleGroupId },
    #[error("SVG style group {group:?} is mapped more than once")]
    DuplicateGroupMapping { group: SvgStyleGroupId },
    #[error("mapping references unknown SVG style group {group:?}")]
    UnknownGroupMapping { group: SvgStyleGroupId },
    #[error("boundary candidate {candidate:?} is not present")]
    UnknownBoundaryCandidate { candidate: SvgBoundaryCandidateId },
    #[error("a boundary candidate cannot be combined with a Boundary style-group mapping")]
    BoundaryCandidateMappingConflict,
    #[error("scaled SVG coordinate is not finite")]
    ScaledCoordinateNotFinite,
    #[error("scaled SVG geometry collapses two distinct source vertices")]
    ScaledVertexCollision,
    #[error("SVG intersection validation exceeds {maximum} candidate pairs")]
    TooManyIntersectionCandidates { maximum: usize },
    #[error("SVG segments {first} and {second} overlap collinearly")]
    CollinearOverlap { first: usize, second: usize },
    #[error("SVG segment intersection cannot be represented safely")]
    IntersectionNotRepresentable,
    #[error("planarized SVG has more than {maximum} edges")]
    TooManyFinalEdges { maximum: usize },
    #[error("planarized SVG has more than {maximum} vertices")]
    TooManyFinalVertices { maximum: usize },
    #[error("planarized SVG contains a degenerate or duplicate edge")]
    InvalidFinalEdge,
    #[error("SVG boundary needs at least three edges, found {actual}")]
    BoundaryTooSmall { actual: usize },
    #[error("SVG boundary has more than {maximum} edges")]
    TooManyBoundaryEdges { maximum: usize },
    #[error("SVG boundary vertex has degree {degree}, expected 2")]
    BoundaryDegree { degree: usize },
    #[error("SVG boundary consists of multiple cycles")]
    BoundaryDisconnected,
    #[error("SVG boundary has zero area")]
    BoundaryZeroArea,
    #[error("SVG boundary self-intersects")]
    BoundarySelfIntersection,
    #[error("SVG boundary geometry cannot be classified safely")]
    BoundaryGeometryUnrepresentable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn multiply(self, rhs: Self) -> Self {
        Self {
            a: self.a * rhs.a + self.c * rhs.b,
            b: self.b * rhs.a + self.d * rhs.b,
            c: self.a * rhs.c + self.c * rhs.d,
            d: self.b * rhs.c + self.d * rhs.d,
            e: self.a * rhs.e + self.c * rhs.f + self.e,
            f: self.b * rhs.e + self.d * rhs.f + self.f,
        }
    }

    fn apply(self, point: Point2) -> Result<Point2, SvgImportError> {
        let point = Point2::new(
            self.a * point.x + self.c * point.y + self.e,
            self.b * point.x + self.d * point.y + self.f,
        );
        if point.x.is_finite() && point.y.is_finite() {
            Ok(point)
        } else {
            Err(SvgImportError::TransformedCoordinateNotFinite)
        }
    }

    fn is_finite_and_invertible(self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .into_iter()
            .all(f64::is_finite)
            && (self.a * self.d - self.b * self.c).is_finite()
            && self.a * self.d - self.b * self.c != 0.0
    }
}

#[derive(Debug, Clone)]
struct StyleDeclaration {
    value: String,
    important: bool,
}

impl StyleDeclaration {
    fn normal(value: String) -> Self {
        Self {
            value,
            important: false,
        }
    }
}

fn cascade_style_declaration(target: &mut Option<StyleDeclaration>, candidate: StyleDeclaration) {
    let should_replace = target
        .as_ref()
        .is_none_or(|current| candidate.important || !current.important);
    if should_replace {
        *target = Some(candidate);
    }
}

#[derive(Debug, Clone, Default)]
struct StyleDeclarations {
    stroke: Option<StyleDeclaration>,
    color: Option<StyleDeclaration>,
    fill: Option<StyleDeclaration>,
    stroke_width: Option<StyleDeclaration>,
    stroke_dasharray: Option<StyleDeclaration>,
    stroke_opacity: Option<StyleDeclaration>,
    opacity: Option<StyleDeclaration>,
    display: Option<StyleDeclaration>,
    visibility: Option<StyleDeclaration>,
}

impl StyleDeclarations {
    fn overlay(&mut self, other: Self) {
        macro_rules! overlay {
            ($field:ident) => {
                if let Some(candidate) = other.$field {
                    cascade_style_declaration(&mut self.$field, candidate);
                }
            };
        }
        overlay!(stroke);
        overlay!(color);
        overlay!(fill);
        overlay!(stroke_width);
        overlay!(stroke_dasharray);
        overlay!(stroke_opacity);
        overlay!(opacity);
        overlay!(display);
        overlay!(visibility);
    }
}

#[derive(Default)]
struct StyleDeclarationRefs<'a> {
    stroke: Option<&'a StyleDeclaration>,
    color: Option<&'a StyleDeclaration>,
    fill: Option<&'a StyleDeclaration>,
    stroke_width: Option<&'a StyleDeclaration>,
    stroke_dasharray: Option<&'a StyleDeclaration>,
    stroke_opacity: Option<&'a StyleDeclaration>,
    opacity: Option<&'a StyleDeclaration>,
    display: Option<&'a StyleDeclaration>,
    visibility: Option<&'a StyleDeclaration>,
}

impl<'a> StyleDeclarationRefs<'a> {
    fn overlay(&mut self, other: &'a StyleDeclarations) {
        macro_rules! overlay {
            ($field:ident) => {
                if let Some(candidate) = other.$field.as_ref() {
                    let should_replace = self
                        .$field
                        .is_none_or(|current| candidate.important || !current.important);
                    if should_replace {
                        self.$field = Some(candidate);
                    }
                }
            };
        }
        overlay!(stroke);
        overlay!(color);
        overlay!(fill);
        overlay!(stroke_width);
        overlay!(stroke_dasharray);
        overlay!(stroke_opacity);
        overlay!(opacity);
        overlay!(display);
        overlay!(visibility);
    }

    fn apply_to(self, target: &mut StyleDeclarations) {
        macro_rules! apply {
            ($field:ident) => {
                if let Some(candidate) = self.$field {
                    let should_replace = target
                        .$field
                        .as_ref()
                        .is_none_or(|current| candidate.important || !current.important);
                    if should_replace {
                        target.$field = Some(candidate.clone());
                    }
                }
            };
        }
        apply!(stroke);
        apply!(color);
        apply!(fill);
        apply!(stroke_width);
        apply!(stroke_dasharray);
        apply!(stroke_opacity);
        apply!(opacity);
        apply!(display);
        apply!(visibility);
    }
}

#[derive(Debug, Clone)]
struct ComputedStyle {
    stroke: String,
    color: String,
    fill: String,
    stroke_width: String,
    stroke_dasharray: String,
    stroke_opacity: String,
    visibility: String,
    opacity_product: f64,
    hidden: bool,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            stroke: "none".to_owned(),
            color: "black".to_owned(),
            fill: "black".to_owned(),
            stroke_width: "1".to_owned(),
            stroke_dasharray: "none".to_owned(),
            stroke_opacity: "1".to_owned(),
            visibility: "visible".to_owned(),
            opacity_product: 1.0,
            hidden: false,
        }
    }
}

impl ComputedStyle {
    fn child(parent: &Self, declarations: &StyleDeclarations) -> Result<Self, SvgImportError> {
        let inherited = |value: &Option<StyleDeclaration>, parent: &str| match value
            .as_ref()
            .map(|declaration| declaration.value.as_str())
        {
            None | Some("inherit") => parent.to_owned(),
            Some(value) => value.to_owned(),
        };
        let local_opacity = parse_opacity(
            declarations
                .opacity
                .as_ref()
                .map_or("1", |declaration| declaration.value.as_str()),
        )?;
        let display_none = declarations
            .display
            .as_ref()
            .is_some_and(|declaration| declaration.value == "none");
        Ok(Self {
            stroke: inherited(&declarations.stroke, &parent.stroke),
            color: inherited(&declarations.color, &parent.color),
            fill: inherited(&declarations.fill, &parent.fill),
            stroke_width: inherited(&declarations.stroke_width, &parent.stroke_width),
            stroke_dasharray: inherited(&declarations.stroke_dasharray, &parent.stroke_dasharray),
            stroke_opacity: inherited(&declarations.stroke_opacity, &parent.stroke_opacity),
            visibility: inherited(&declarations.visibility, &parent.visibility),
            opacity_product: parent.opacity_product * local_opacity,
            hidden: parent.hidden || display_none,
        })
    }
}

#[derive(Debug, Clone)]
struct CssRule {
    class: String,
    declarations: StyleDeclarations,
}

#[derive(Debug, Clone)]
struct ElementContext {
    name: String,
    is_svg_namespace: bool,
    ctm: Affine,
    style: ComputedStyle,
    skip_geometry: bool,
    representative_id: Option<String>,
    layer: Option<String>,
    semantic: Option<String>,
    classes: Vec<String>,
}

impl Default for ElementContext {
    fn default() -> Self {
        Self {
            name: String::new(),
            is_svg_namespace: false,
            ctm: Affine::IDENTITY,
            style: ComputedStyle::default(),
            skip_geometry: false,
            representative_id: None,
            layer: None,
            semantic: None,
            classes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StyleKey {
    stroke: [u8; 4],
    stroke_width: u64,
    dash: Vec<u64>,
    classes: Vec<String>,
    layer: Option<String>,
    semantic: Option<String>,
}

struct WarningCollector {
    warnings: BTreeMap<SvgWarningKind, usize>,
    maximum: usize,
}

impl WarningCollector {
    fn new(maximum: usize) -> Self {
        Self {
            warnings: BTreeMap::new(),
            maximum,
        }
    }

    fn add(&mut self, kind: SvgWarningKind) -> Result<(), SvgImportError> {
        if let Some(count) = self.warnings.get_mut(&kind) {
            *count = count.saturating_add(1);
            return Ok(());
        }
        if self.warnings.len() == self.maximum {
            return Err(SvgImportError::TooManyWarnings {
                maximum: self.maximum,
            });
        }
        self.warnings.insert(kind, 1);
        Ok(())
    }

    fn into_vec(self) -> Vec<SvgPreviewWarning> {
        self.warnings
            .into_iter()
            .map(|(kind, occurrences)| SvgPreviewWarning { kind, occurrences })
            .collect()
    }
}

struct SvgParser {
    limits: SvgImportLimits,
    css_rules: Vec<CssRule>,
    warnings: WarningCollector,
    vertices: Vec<SvgPreviewVertex>,
    position_indices: HashMap<(u64, u64), usize>,
    edges: Vec<SvgPreviewEdge>,
    style_groups: Vec<SvgStyleGroup>,
    style_indices: HashMap<StyleKey, SvgStyleGroupId>,
    boundary_candidates: Vec<SvgBoundaryCandidate>,
    boundary_candidate_intersection_count: usize,
    element_count: usize,
    path_command_count: usize,
    css_rule_element_evaluation_count: usize,
    transform_count: usize,
    title: String,
    view_box: Option<ViewBox>,
    root_width: Option<f64>,
    root_height: Option<f64>,
    root_width_unit: Option<SvgRootLengthUnit>,
    root_height_unit: Option<SvgRootLengthUnit>,
    root_css_pixels_assumed: bool,
}

impl SvgParser {
    fn new(limits: SvgImportLimits, css_rules: Vec<CssRule>, warnings: WarningCollector) -> Self {
        Self {
            limits,
            css_rules,
            warnings,
            vertices: Vec::new(),
            position_indices: HashMap::new(),
            edges: Vec::new(),
            style_groups: Vec::new(),
            style_indices: HashMap::new(),
            boundary_candidates: Vec::new(),
            boundary_candidate_intersection_count: 0,
            element_count: 0,
            path_command_count: 0,
            css_rule_element_evaluation_count: 0,
            transform_count: 0,
            title: String::new(),
            view_box: None,
            root_width: None,
            root_height: None,
            root_width_unit: None,
            root_height_unit: None,
            root_css_pixels_assumed: false,
        }
    }

    fn finish(mut self) -> Result<SvgPreview, SvgImportError> {
        self.compact_style_groups();
        if self.edges.is_empty() {
            return Err(SvgImportError::NoSupportedGeometry);
        }
        let recommended = self.recommended_scale();
        if recommended.is_none() {
            self.warnings
                .add(SvgWarningKind::PhysicalScaleNeedsSelection)?;
        }
        if self.root_css_pixels_assumed {
            self.warnings.add(SvgWarningKind::CssPixelScaleAssumed)?;
        }
        let root_view_box = self.view_box.map(|view_box| SvgRootViewBox {
            x: view_box.x,
            y: view_box.y,
            width: view_box.w,
            height: view_box.h,
        });
        Ok(SvgPreview {
            title: (!self.title.is_empty()).then_some(self.title),
            root_view_box,
            root_physical_size: SvgRootPhysicalSize {
                width_millimetres: self.root_width,
                height_millimetres: self.root_height,
                width_unit: self.root_width_unit,
                height_unit: self.root_height_unit,
            },
            recommended_millimetres_per_unit: recommended,
            vertices: self.vertices,
            edges: self.edges,
            style_groups: self.style_groups,
            boundary_candidates: self.boundary_candidates,
            warnings: self.warnings.into_vec(),
            limits: self.limits,
        })
    }

    fn compact_style_groups(&mut self) {
        let mut remap = vec![None; self.style_groups.len()];
        let mut compacted = Vec::new();
        for mut group in self.style_groups.drain(..) {
            if group.segment_count == 0 {
                continue;
            }
            let id = SvgStyleGroupId(
                u16::try_from(compacted.len()).expect("style group limit is below u16::MAX"),
            );
            remap[usize::from(group.id.0)] = Some(id);
            group.id = id;
            compacted.push(group);
        }
        for edge in &mut self.edges {
            edge.style_group = remap[usize::from(edge.style_group.0)]
                .expect("every retained edge has a non-empty style group");
        }
        self.style_groups = compacted;
    }

    fn recommended_scale(&self) -> Option<f64> {
        let view_box = self.view_box?;
        let width = self.root_width?;
        let height = self.root_height?;
        let sx = width / view_box.w;
        let sy = height / view_box.h;
        if !sx.is_finite() || !sy.is_finite() || sx <= 0.0 || sy <= 0.0 {
            return None;
        }
        let tolerance = sx.abs().max(sy.abs()) * 1e-12;
        ((sx - sy).abs() <= tolerance).then_some(sx + (sy - sx) * 0.5)
    }

    fn record_css_rule_element_evaluations(&mut self, count: usize) -> Result<(), SvgImportError> {
        self.css_rule_element_evaluation_count = self
            .css_rule_element_evaluation_count
            .checked_add(count)
            .ok_or(SvgImportError::TooManyCssRuleElementEvaluations {
                maximum: self.limits.max_css_rule_element_evaluations,
            })?;
        if self.css_rule_element_evaluation_count > self.limits.max_css_rule_element_evaluations {
            return Err(SvgImportError::TooManyCssRuleElementEvaluations {
                maximum: self.limits.max_css_rule_element_evaluations,
            });
        }
        Ok(())
    }

    fn add_vertex(&mut self, position: Point2) -> Result<usize, SvgImportError> {
        let key = position_key(position);
        if let Some(index) = self.position_indices.get(&key) {
            return Ok(*index);
        }
        if self.vertices.len() == self.limits.max_source_vertices {
            return Err(SvgImportError::TooManySourceVertices {
                maximum: self.limits.max_source_vertices,
            });
        }
        let index = self.vertices.len();
        self.vertices.push(SvgPreviewVertex { index, position });
        self.position_indices.insert(key, index);
        Ok(index)
    }

    fn add_edge(
        &mut self,
        start: Point2,
        end: Point2,
        group: SvgStyleGroupId,
        element: &str,
    ) -> Result<usize, SvgImportError> {
        if start == end {
            return Err(SvgImportError::DegenerateSegment {
                element: element.to_owned(),
            });
        }
        if !(end.x - start.x).is_finite() || !(end.y - start.y).is_finite() {
            return Err(SvgImportError::TransformedCoordinateNotFinite);
        }
        if self.edges.len() == self.limits.max_source_edges {
            return Err(SvgImportError::TooManySourceEdges {
                maximum: self.limits.max_source_edges,
            });
        }
        let vertices = [self.add_vertex(start)?, self.add_vertex(end)?];
        let index = self.edges.len();
        self.edges.push(SvgPreviewEdge {
            index,
            vertices,
            style_group: group,
        });
        self.style_groups[usize::from(group.0)].segment_count += 1;
        Ok(index)
    }

    fn add_candidate(
        &mut self,
        kind: SvgBoundaryCandidateKind,
        positions: &[Point2],
        source_edges: Vec<usize>,
    ) -> Result<(), SvgImportError> {
        if positions.len() < 3 {
            return Ok(());
        }
        let pair_count = positions
            .len()
            .checked_mul(positions.len().saturating_sub(3))
            .and_then(|count| count.checked_div(2))
            .ok_or(SvgImportError::TooManyBoundaryCandidateIntersections {
                maximum: self.limits.max_intersection_candidates,
            })?;
        self.boundary_candidate_intersection_count = self
            .boundary_candidate_intersection_count
            .checked_add(pair_count)
            .ok_or(SvgImportError::TooManyBoundaryCandidateIntersections {
                maximum: self.limits.max_intersection_candidates,
            })?;
        if self.boundary_candidate_intersection_count > self.limits.max_intersection_candidates {
            return Err(SvgImportError::TooManyBoundaryCandidateIntersections {
                maximum: self.limits.max_intersection_candidates,
            });
        }
        if !is_simple_nonzero_polygon(positions) {
            return Ok(());
        }
        let maximum = self
            .limits
            .max_boundary_candidates
            .min(usize::from(u16::MAX) + 1);
        if self.boundary_candidates.len() == maximum {
            return Err(SvgImportError::TooManyBoundaryCandidates { maximum });
        }
        let vertex_indices = positions
            .iter()
            .copied()
            .map(|point| self.add_vertex(point))
            .collect::<Result<Vec<_>, _>>()?;
        let id = SvgBoundaryCandidateId(
            u16::try_from(self.boundary_candidates.len())
                .expect("boundary candidate limit is below u16::MAX"),
        );
        self.boundary_candidates.push(SvgBoundaryCandidate {
            id,
            kind,
            vertex_indices,
            source_edge_indices: source_edges,
        });
        Ok(())
    }

    fn style_group(
        &mut self,
        context: &ElementContext,
    ) -> Result<Option<SvgStyleGroupId>, SvgImportError> {
        if context.skip_geometry
            || context.style.hidden
            || context.style.visibility == "hidden"
            || context.style.visibility == "collapse"
        {
            self.warnings.add(SvgWarningKind::HiddenGeometryIgnored)?;
            return Ok(None);
        }

        let paint = match Paint::from_str(&context.style.stroke) {
            Ok(Paint::Color(color)) => color,
            Ok(Paint::CurrentColor) => match Paint::from_str(&context.style.color) {
                Ok(Paint::Color(color)) => color,
                _ => {
                    self.warnings
                        .add(SvgWarningKind::UnsupportedPaint(bounded_detail(&format!(
                            "color:{}",
                            context.style.color
                        ))))?;
                    return Ok(None);
                }
            },
            Ok(Paint::None) => {
                self.warnings
                    .add(SvgWarningKind::GeometryWithoutStrokeIgnored)?;
                return Ok(None);
            }
            Ok(_) | Err(_) => {
                self.warnings
                    .add(SvgWarningKind::UnsupportedPaint(bounded_detail(
                        &context.style.stroke,
                    )))?;
                return Ok(None);
            }
        };
        let width = match parse_supported_length(&context.style.stroke_width) {
            Ok(value) if value > 0.0 => value,
            _ => {
                self.warnings
                    .add(SvgWarningKind::UnsupportedLengthUnit(bounded_detail(
                        &context.style.stroke_width,
                    )))?;
                return Ok(None);
            }
        };
        let stroke_opacity = parse_opacity(&context.style.stroke_opacity)?;
        let alpha = f64::from(paint.alpha) / 255.0 * stroke_opacity * context.style.opacity_product;
        if alpha <= 0.0 {
            self.warnings
                .add(SvgWarningKind::GeometryWithoutStrokeIgnored)?;
            return Ok(None);
        }
        let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        let dash = match parse_dash_pattern(&context.style.stroke_dasharray) {
            Ok(pattern) => pattern,
            Err(()) => {
                self.warnings
                    .add(SvgWarningKind::UnsupportedLengthUnit(bounded_detail(
                        &context.style.stroke_dasharray,
                    )))?;
                return Ok(None);
            }
        };
        if !matches!(Paint::from_str(&context.style.fill), Ok(Paint::None)) {
            self.warnings.add(SvgWarningKind::FillIgnored)?;
        }

        let stroke = RgbaColor {
            red: paint.red,
            green: paint.green,
            blue: paint.blue,
            alpha,
        };
        let mut classes = context.classes.clone();
        classes.sort();
        classes.dedup();
        let dash_bits = match &dash {
            SvgDashPattern::Solid => Vec::new(),
            SvgDashPattern::Dashes(values) => values
                .iter()
                .map(|value| canonical_f64_bits(*value))
                .collect(),
        };
        let key = StyleKey {
            stroke: [stroke.red, stroke.green, stroke.blue, stroke.alpha],
            stroke_width: canonical_f64_bits(width),
            dash: dash_bits,
            classes: classes.clone(),
            layer: context.layer.clone(),
            semantic: context.semantic.clone(),
        };
        if let Some(group) = self.style_indices.get(&key) {
            return Ok(Some(*group));
        }
        let maximum = self.limits.max_style_groups.min(usize::from(u16::MAX) + 1);
        if self.style_groups.len() == maximum {
            return Err(SvgImportError::TooManyStyleGroups { maximum });
        }
        let group = SvgStyleGroupId(
            u16::try_from(self.style_groups.len()).expect("style group limit is below u16::MAX"),
        );
        self.style_groups.push(SvgStyleGroup {
            id: group,
            segment_count: 0,
            element_count: 0,
            representative_id: None,
            stroke,
            stroke_width: width,
            dash_pattern: dash,
            classes,
            layer: context.layer.clone(),
            semantic: context.semantic.clone(),
        });
        self.style_indices.insert(key, group);
        Ok(Some(group))
    }
}

/// Reads a preview with conservative desktop defaults.
pub fn read_svg_preview(bytes: &[u8]) -> Result<SvgPreview, SvgImportError> {
    read_svg_preview_with_limits(bytes, SvgImportLimits::default())
}

/// Reads and validates the supported static straight-line SVG subset.
pub fn read_svg_preview_with_limits(
    bytes: &[u8],
    limits: SvgImportLimits,
) -> Result<SvgPreview, SvgImportError> {
    if bytes.len() > limits.max_file_bytes {
        return Err(SvgImportError::FileTooLarge {
            actual: bytes.len(),
            maximum: limits.max_file_bytes,
        });
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(|_| SvgImportError::NonUtf8)?;

    let mut warnings = WarningCollector::new(limits.max_warnings);
    let css_rules = collect_css_rules(bytes, limits, &mut warnings)?;
    let mut parser = SvgParser::new(limits, css_rules, warnings);
    parse_svg_document(bytes, &mut parser)?;
    parser.finish()
}

fn namespace_is_svg(namespace: &ResolveResult<'_>) -> bool {
    matches!(
        namespace,
        ResolveResult::Bound(Namespace(uri)) if *uri == SVG_NAMESPACE.as_bytes()
    )
}

fn collect_css_rules(
    bytes: &[u8],
    limits: SvgImportLimits,
    warnings: &mut WarningCollector,
) -> Result<Vec<CssRule>, SvgImportError> {
    let mut reader = NsReader::from_reader(bytes);
    reader.config_mut().check_comments = true;
    reader.config_mut().check_end_names = true;
    reader.config_mut().allow_unmatched_ends = false;
    reader
        .resolver_mut()
        .set_max_declarations_per_element(limits.max_attributes_per_element);
    let mut inside_style = 0_usize;
    let mut style_text = String::new();
    let mut rules = Vec::new();
    let mut depth = 0_usize;
    let mut element_count = 0_usize;
    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|error| SvgImportError::InvalidXml(error.to_string()))?;
        let is_svg_namespace = namespace_is_svg(&namespace);
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                element_count = element_count.saturating_add(1);
                if element_count > limits.max_elements {
                    return Err(SvgImportError::TooManyElements {
                        maximum: limits.max_elements,
                    });
                }
                let event_depth = depth + 1;
                if event_depth > limits.max_depth {
                    return Err(SvgImportError::TooDeep {
                        actual: event_depth,
                        maximum: limits.max_depth,
                    });
                }
                // This first pass must enforce the same attribute bound before
                // collecting stylesheet text.
                read_attributes(element, limits)?;
                let is_start = matches!(&event, Event::Start(_));
                if is_start {
                    depth = event_depth;
                }
                if is_start
                    && (inside_style > 0
                        || (is_svg_namespace && element.local_name().as_ref() == b"style"))
                {
                    inside_style += 1;
                }
            }
            Event::End(_) if inside_style > 0 => {
                inside_style -= 1;
                depth = depth.saturating_sub(1);
                if inside_style == 0 {
                    parse_css_text(&style_text, limits, warnings, &mut rules)?;
                    style_text.clear();
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::Text(text) if inside_style > 0 => {
                let decoded = text.decode().map_err(|_| SvgImportError::NonUtf8)?;
                style_text.push_str(&decoded);
                if style_text.len() > MAX_STYLE_TEXT_BYTES {
                    return Err(SvgImportError::InvalidCss);
                }
            }
            Event::CData(text) if inside_style > 0 => {
                let decoded = text.decode().map_err(|_| SvgImportError::NonUtf8)?;
                style_text.push_str(&decoded);
                if style_text.len() > MAX_STYLE_TEXT_BYTES {
                    return Err(SvgImportError::InvalidCss);
                }
            }
            Event::Decl(declaration) => validate_xml_declaration(&declaration)?,
            Event::DocType(_) => return Err(SvgImportError::DoctypeNotAllowed),
            Event::GeneralRef(reference) if inside_style > 0 => {
                append_xml_reference(&mut style_text, &reference)?;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(rules)
}

fn parse_svg_document(bytes: &[u8], parser: &mut SvgParser) -> Result<(), SvgImportError> {
    let mut reader = NsReader::from_reader(bytes);
    reader.config_mut().check_comments = true;
    reader.config_mut().check_end_names = true;
    reader.config_mut().allow_unmatched_ends = false;
    reader
        .resolver_mut()
        .set_max_declarations_per_element(parser.limits.max_attributes_per_element);
    let mut stack = Vec::<ElementContext>::new();
    let mut saw_root = false;
    let mut closed_root = false;
    let mut declaration_seen = false;
    let mut prolog_content_seen = false;

    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|error| SvgImportError::InvalidXml(error.to_string()))?;
        if matches!(namespace, ResolveResult::Unknown(_)) {
            return Err(SvgImportError::InvalidXml(
                "an element uses an undeclared namespace prefix".to_owned(),
            ));
        }
        let is_svg_namespace = namespace_is_svg(&namespace);
        match &event {
            Event::Start(element) | Event::Empty(element) => {
                prolog_content_seen = true;
                if closed_root {
                    return Err(SvgImportError::TrailingContent);
                }
                parser.element_count = parser.element_count.saturating_add(1);
                if parser.element_count > parser.limits.max_elements {
                    return Err(SvgImportError::TooManyElements {
                        maximum: parser.limits.max_elements,
                    });
                }
                let is_empty = matches!(&event, Event::Empty(_));
                let depth = stack.len() + 1;
                if depth > parser.limits.max_depth {
                    return Err(SvgImportError::TooDeep {
                        actual: depth,
                        maximum: parser.limits.max_depth,
                    });
                }
                let raw_name = std::str::from_utf8(element.name().as_ref())
                    .map_err(|_| SvgImportError::NonUtf8)?
                    .to_owned();
                let local_name = std::str::from_utf8(element.local_name().as_ref())
                    .map_err(|_| SvgImportError::NonUtf8)?
                    .to_owned();
                let attributes = read_attributes(element, parser.limits)?;

                if !saw_root {
                    if raw_name != "svg" {
                        return Err(SvgImportError::MissingSvgRoot);
                    }
                    if !is_svg_namespace {
                        return Err(SvgImportError::InvalidSvgNamespace);
                    }
                    saw_root = true;
                }

                let parent = stack.last().cloned().unwrap_or_default();
                let context = build_context(
                    parser,
                    &parent,
                    &local_name,
                    &raw_name,
                    &attributes,
                    depth,
                    is_svg_namespace,
                )?;
                if depth == 1 {
                    parse_root_geometry(parser, &context, &attributes)?;
                }
                if is_shape_name(&local_name) && !context.skip_geometry {
                    parse_shape(parser, &context, &attributes)?;
                }
                if !is_empty {
                    stack.push(context);
                } else if depth == 1 {
                    closed_root = true;
                }
            }
            Event::End(_) => {
                let Some(context) = stack.pop() else {
                    return Err(SvgImportError::InvalidXml(
                        "unexpected closing element".to_owned(),
                    ));
                };
                if stack.is_empty() && context.name == "svg" {
                    closed_root = true;
                }
            }
            Event::Text(text) => {
                let decoded = text.decode().map_err(|_| SvgImportError::NonUtf8)?;
                if stack.is_empty() {
                    if !decoded.trim().is_empty() {
                        return Err(SvgImportError::TrailingContent);
                    }
                    if !saw_root {
                        prolog_content_seen = true;
                    }
                } else if stack.len() == 2
                    && stack
                        .last()
                        .is_some_and(|context| context.name == "title" && context.is_svg_namespace)
                {
                    parser.title.push_str(&decoded);
                    if parser.title.chars().count() > MAX_TITLE_CHARS {
                        return Err(SvgImportError::TitleTooLong {
                            maximum: MAX_TITLE_CHARS,
                        });
                    }
                } else if !decoded.trim().is_empty()
                    && !stack
                        .last()
                        .is_some_and(|context| context.name == "style" && context.is_svg_namespace)
                {
                    parser.warnings.add(SvgWarningKind::MetadataIgnored)?;
                }
            }
            Event::CData(_) => {
                if stack.is_empty() {
                    return Err(SvgImportError::TrailingContent);
                }
                if !stack
                    .last()
                    .is_some_and(|context| context.name == "style" && context.is_svg_namespace)
                {
                    parser.warnings.add(SvgWarningKind::MetadataIgnored)?;
                }
            }
            Event::GeneralRef(reference) => {
                if stack.is_empty() {
                    return Err(SvgImportError::TrailingContent);
                }
                if stack.len() == 2
                    && stack
                        .last()
                        .is_some_and(|context| context.name == "title" && context.is_svg_namespace)
                {
                    append_xml_reference(&mut parser.title, reference)?;
                    if parser.title.chars().count() > MAX_TITLE_CHARS {
                        return Err(SvgImportError::TitleTooLong {
                            maximum: MAX_TITLE_CHARS,
                        });
                    }
                } else {
                    validate_xml_reference(reference)?;
                }
            }
            Event::Decl(declaration) => {
                if declaration_seen || saw_root || prolog_content_seen {
                    return Err(SvgImportError::InvalidXml(
                        "XML declaration must occur once at the start".to_owned(),
                    ));
                }
                declaration_seen = true;
                validate_xml_declaration(declaration)?;
            }
            Event::DocType(_) => return Err(SvgImportError::DoctypeNotAllowed),
            Event::PI(_) => {
                if !saw_root {
                    prolog_content_seen = true;
                }
                parser.warnings.add(SvgWarningKind::MetadataIgnored)?;
            }
            Event::Comment(_) => {
                if !saw_root {
                    prolog_content_seen = true;
                }
            }
            Event::Eof => break,
        }
    }
    if !saw_root || !closed_root {
        return Err(SvgImportError::MissingSvgRoot);
    }
    Ok(())
}

fn read_attributes(
    element: &BytesStart<'_>,
    limits: SvgImportLimits,
) -> Result<HashMap<String, String>, SvgImportError> {
    let mut attributes = HashMap::new();
    let mut count = 0_usize;
    for attribute in element.attributes() {
        count += 1;
        if count > limits.max_attributes_per_element {
            return Err(SvgImportError::TooManyAttributes {
                maximum: limits.max_attributes_per_element,
            });
        }
        let attribute = attribute.map_err(|error| SvgImportError::InvalidXml(error.to_string()))?;
        let key = std::str::from_utf8(attribute.key.as_ref())
            .map_err(|_| SvgImportError::NonUtf8)?
            .to_owned();
        let value = attribute
            .normalized_value(XmlVersion::Explicit1_0)
            .map_err(|error| SvgImportError::InvalidXml(error.to_string()))?
            .into_owned();
        attributes.insert(key, value);
    }
    Ok(attributes)
}

fn build_context(
    parser: &mut SvgParser,
    parent: &ElementContext,
    local_name: &str,
    raw_name: &str,
    attributes: &HashMap<String, String>,
    depth: usize,
    is_svg_namespace: bool,
) -> Result<ElementContext, SvgImportError> {
    let local_classes = parse_classes(attributes.get("class").map(String::as_str))?;
    let local_class_set = local_classes
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    parser.record_css_rule_element_evaluations(parser.css_rules.len())?;
    let mut declarations = presentation_declarations(attributes)?;
    let mut css_declarations = StyleDeclarationRefs::default();
    for rule in &parser.css_rules {
        if local_class_set.contains(rule.class.as_str()) {
            css_declarations.overlay(&rule.declarations);
        }
    }
    css_declarations.apply_to(&mut declarations);
    if let Some(style) = attributes.get("style") {
        declarations.overlay(parse_declarations(
            style,
            &mut parser.warnings,
            DeclarationOrigin::Inline,
        )?);
    }
    let style = ComputedStyle::child(&parent.style, &declarations)?;
    let local_transform = match attributes.get("transform") {
        Some(transform) => parse_transform(transform, parser)?,
        None => Affine::IDENTITY,
    };
    let ctm = parent.ctm.multiply(local_transform);
    if !ctm.is_finite_and_invertible() {
        return Err(SvgImportError::InvalidTransform);
    }
    let local_layer = (is_svg_namespace && local_name == "g")
        .then(|| {
            attributes
                .get("data-origami-layer")
                .or_else(|| attributes.get("inkscape:label"))
                .or_else(|| attributes.get("data-layer"))
                .or_else(|| attributes.get("id"))
                .map(String::as_str)
        })
        .flatten()
        .map(|value| semantic_hint(Some(value)))
        .transpose()?
        .flatten();
    let layer = append_layer_path(parent.layer.as_deref(), local_layer.as_deref())?;
    let semantic = match attributes.get("data-origami-kind") {
        Some(value) => match canonical_origami_kind(value) {
            Some(value) => Some(value.to_owned()),
            None => {
                parser.warnings.add(SvgWarningKind::UnsupportedAttribute(
                    "data-origami-kind".to_owned(),
                ))?;
                None
            }
        },
        None => parent.semantic.clone(),
    };
    let mut classes = parent.classes.clone();
    for class in local_classes {
        if !classes.contains(&class) {
            if classes.len() == MAX_CLASS_TOKENS {
                return Err(SvgImportError::InvalidAttribute {
                    element: local_name.to_owned(),
                    attribute: "class".to_owned(),
                });
            }
            classes.push(class);
        }
    }

    let supported_container = is_svg_namespace && matches!(local_name, "svg" | "g" | "a");
    let metadata =
        is_svg_namespace && matches!(local_name, "style" | "title" | "desc" | "metadata" | "defs");
    let shape = is_svg_namespace && is_shape_name(local_name);
    let representative_id = shape
        .then(|| representative_id_hint(attributes.get("id").map(String::as_str)))
        .flatten();
    let unsupported =
        (!supported_container && !metadata && !shape) || (local_name == "svg" && depth != 1);
    if unsupported {
        parser
            .warnings
            .add(SvgWarningKind::UnsupportedElement(bounded_detail(raw_name)))?;
    }
    if metadata && local_name != "title" && local_name != "style" {
        parser.warnings.add(SvgWarningKind::MetadataIgnored)?;
    }
    if attributes.keys().any(|name| {
        name == "href"
            || name.ends_with(":href")
            || name == "src"
            || name == "xml:base"
            || name.starts_with("on")
    }) {
        parser
            .warnings
            .add(SvgWarningKind::ExternalReferenceIgnored)?;
    }
    warn_unknown_attributes(parser, local_name, attributes)?;

    Ok(ElementContext {
        name: local_name.to_owned(),
        is_svg_namespace,
        ctm,
        style,
        skip_geometry: parent.skip_geometry || unsupported || metadata,
        representative_id,
        layer,
        semantic,
        classes,
    })
}

fn parse_root_geometry(
    parser: &mut SvgParser,
    context: &ElementContext,
    attributes: &HashMap<String, String>,
) -> Result<(), SvgImportError> {
    parser.view_box = match attributes.get("viewBox") {
        Some(value) => {
            Some(
                parse_view_box(value).map_err(|()| SvgImportError::InvalidAttribute {
                    element: "svg".to_owned(),
                    attribute: "viewBox".to_owned(),
                })?,
            )
        }
        None => None,
    };
    let width = attributes
        .get("width")
        .map(|value| parse_root_physical_length(value))
        .transpose()?;
    let height = attributes
        .get("height")
        .map(|value| parse_root_physical_length(value))
        .transpose()?;
    parser.root_width = width.and_then(|length| length.millimetres);
    parser.root_height = height.and_then(|length| length.millimetres);
    parser.root_width_unit = width.map(|length| length.unit);
    parser.root_height_unit = height.map(|length| length.unit);
    parser.root_css_pixels_assumed = width.is_some_and(|length| length.css_pixels_assumed)
        || height.is_some_and(|length| length.css_pixels_assumed);
    if let Some(value) = attributes.get("preserveAspectRatio") {
        parse_aspect_ratio(value).map_err(|()| SvgImportError::InvalidAttribute {
            element: "svg".to_owned(),
            attribute: "preserveAspectRatio".to_owned(),
        })?;
    }

    if let Some(view_box) = parser.view_box {
        let positions = [
            Point2::new(view_box.x, view_box.y),
            Point2::new(view_box.x + view_box.w, view_box.y),
            Point2::new(view_box.x + view_box.w, view_box.y + view_box.h),
            Point2::new(view_box.x, view_box.y + view_box.h),
        ]
        .into_iter()
        .map(|point| context.ctm.apply(point))
        .collect::<Result<Vec<_>, _>>()?;
        parser.add_candidate(SvgBoundaryCandidateKind::ViewBox, &positions, Vec::new())?;
    }
    Ok(())
}

fn parse_shape(
    parser: &mut SvgParser,
    context: &ElementContext,
    attributes: &HashMap<String, String>,
) -> Result<(), SvgImportError> {
    let Some(group) = parser.style_group(context)? else {
        return Ok(());
    };
    let previous_edge_count = parser.edges.len();
    match context.name.as_str() {
        "line" => parse_line(parser, context, attributes, group),
        "polyline" => parse_poly(parser, context, attributes, group, false),
        "polygon" => parse_poly(parser, context, attributes, group, true),
        "rect" => parse_rectangle(parser, context, attributes, group),
        "path" => parse_path(parser, context, attributes, group),
        _ => unreachable!("shape name was checked"),
    }?;
    if parser.edges.len() > previous_edge_count {
        let style_group = &mut parser.style_groups[usize::from(group.0)];
        style_group.element_count += 1;
        if style_group.representative_id.is_none() {
            style_group.representative_id = context.representative_id.clone();
        }
    }
    Ok(())
}

fn parse_line(
    parser: &mut SvgParser,
    context: &ElementContext,
    attributes: &HashMap<String, String>,
    group: SvgStyleGroupId,
) -> Result<(), SvgImportError> {
    let start = context.ctm.apply(Point2::new(
        shape_length(attributes, "x1", 0.0, "line")?,
        shape_length(attributes, "y1", 0.0, "line")?,
    ))?;
    let end = context.ctm.apply(Point2::new(
        shape_length(attributes, "x2", 0.0, "line")?,
        shape_length(attributes, "y2", 0.0, "line")?,
    ))?;
    parser.add_edge(start, end, group, "line")?;
    Ok(())
}

fn parse_poly(
    parser: &mut SvgParser,
    context: &ElementContext,
    attributes: &HashMap<String, String>,
    group: SvgStyleGroupId,
    polygon: bool,
) -> Result<(), SvgImportError> {
    let element = if polygon { "polygon" } else { "polyline" };
    let points_text = attributes
        .get("points")
        .ok_or_else(|| SvgImportError::InvalidAttribute {
            element: element.to_owned(),
            attribute: "points".to_owned(),
        })?;
    if points_text.trim_end().ends_with(',') {
        return Err(SvgImportError::InvalidAttribute {
            element: element.to_owned(),
            attribute: "points".to_owned(),
        });
    }
    let mut numbers = Vec::new();
    for number in NumberListParser::from(points_text.as_str()) {
        let number = number.map_err(|_| SvgImportError::InvalidAttribute {
            element: element.to_owned(),
            attribute: "points".to_owned(),
        })?;
        if !number.is_finite() {
            return Err(SvgImportError::NonFiniteNumber {
                element: element.to_owned(),
            });
        }
        numbers.push(number);
        if numbers.len()
            > parser
                .limits
                .max_source_edges
                .saturating_mul(2)
                .saturating_add(2)
        {
            return Err(SvgImportError::TooManySourceEdges {
                maximum: parser.limits.max_source_edges,
            });
        }
    }
    if numbers.len() % 2 != 0 {
        return Err(SvgImportError::InvalidAttribute {
            element: element.to_owned(),
            attribute: "points".to_owned(),
        });
    }
    let mut points = numbers
        .chunks_exact(2)
        .map(|pair| context.ctm.apply(Point2::new(pair[0], pair[1])))
        .collect::<Result<Vec<_>, _>>()?;
    if points.len() < if polygon { 3 } else { 2 } {
        return Err(SvgImportError::InvalidAttribute {
            element: element.to_owned(),
            attribute: "points".to_owned(),
        });
    }
    let explicitly_closed = !polygon && points.first() == points.last();
    if explicitly_closed {
        points.pop();
    }
    let mut edge_indices = Vec::new();
    for pair in points.windows(2) {
        edge_indices.push(parser.add_edge(pair[0], pair[1], group, element)?);
    }
    if polygon || explicitly_closed {
        edge_indices.push(parser.add_edge(
            *points.last().expect("at least two points"),
            points[0],
            group,
            element,
        )?);
        parser.add_candidate(
            if polygon {
                SvgBoundaryCandidateKind::Polygon
            } else {
                SvgBoundaryCandidateKind::Polyline
            },
            &points,
            edge_indices,
        )?;
    }
    Ok(())
}

fn parse_rectangle(
    parser: &mut SvgParser,
    context: &ElementContext,
    attributes: &HashMap<String, String>,
    group: SvgStyleGroupId,
) -> Result<(), SvgImportError> {
    let rx = shape_length(attributes, "rx", 0.0, "rect")?;
    let ry = shape_length(attributes, "ry", 0.0, "rect")?;
    if rx != 0.0 || ry != 0.0 {
        parser.warnings.add(SvgWarningKind::UnsupportedElement(
            "rounded rect".to_owned(),
        ))?;
        return Ok(());
    }
    let x = shape_length(attributes, "x", 0.0, "rect")?;
    let y = shape_length(attributes, "y", 0.0, "rect")?;
    let width = shape_length(attributes, "width", f64::NAN, "rect")?;
    let height = shape_length(attributes, "height", f64::NAN, "rect")?;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(SvgImportError::InvalidAttribute {
            element: "rect".to_owned(),
            attribute: "width/height".to_owned(),
        });
    }
    let points = [
        Point2::new(x, y),
        Point2::new(x + width, y),
        Point2::new(x + width, y + height),
        Point2::new(x, y + height),
    ]
    .into_iter()
    .map(|point| context.ctm.apply(point))
    .collect::<Result<Vec<_>, _>>()?;
    let mut edge_indices = Vec::new();
    for index in 0..points.len() {
        edge_indices.push(parser.add_edge(
            points[index],
            points[(index + 1) % points.len()],
            group,
            "rect",
        )?);
    }
    parser.add_candidate(SvgBoundaryCandidateKind::Rectangle, &points, edge_indices)
}

fn parse_path(
    parser: &mut SvgParser,
    context: &ElementContext,
    attributes: &HashMap<String, String>,
    group: SvgStyleGroupId,
) -> Result<(), SvgImportError> {
    let data = attributes
        .get("d")
        .ok_or_else(|| SvgImportError::InvalidAttribute {
            element: "path".to_owned(),
            attribute: "d".to_owned(),
        })?;
    let mut commands = Vec::new();
    let mut has_unsupported_command = false;
    for command in PathParser::from(data.as_str()) {
        let command = command.map_err(|_| SvgImportError::InvalidAttribute {
            element: "path".to_owned(),
            attribute: "d".to_owned(),
        })?;
        parser.path_command_count = parser.path_command_count.saturating_add(1);
        if parser.path_command_count > parser.limits.max_path_commands {
            return Err(SvgImportError::TooManyPathCommands {
                maximum: parser.limits.max_path_commands,
            });
        }
        if !matches!(
            command,
            PathSegment::MoveTo { .. }
                | PathSegment::LineTo { .. }
                | PathSegment::HorizontalLineTo { .. }
                | PathSegment::VerticalLineTo { .. }
                | PathSegment::ClosePath { .. }
        ) {
            has_unsupported_command = true;
            parser
                .warnings
                .add(SvgWarningKind::UnsupportedPathCommand(char::from(
                    command.command().to_ascii_uppercase(),
                )))?;
        }
        commands.push(command);
    }
    if has_unsupported_command {
        return Ok(());
    }

    let mut current = Point2::new(0.0, 0.0);
    let mut subpath_start = None;
    let mut contour_points = Vec::<Point2>::new();
    let mut contour_edges = Vec::<usize>::new();
    let mut any_edge = false;
    for command in commands {
        match command {
            PathSegment::MoveTo { abs, x, y } => {
                current = if abs {
                    Point2::new(x, y)
                } else {
                    Point2::new(current.x + x, current.y + y)
                };
                ensure_finite_point(current, "path")?;
                subpath_start = Some(current);
                contour_points.clear();
                contour_points.push(context.ctm.apply(current)?);
                contour_edges.clear();
            }
            PathSegment::LineTo { abs, x, y } => {
                let next = if abs {
                    Point2::new(x, y)
                } else {
                    Point2::new(current.x + x, current.y + y)
                };
                add_path_line(
                    parser,
                    context,
                    group,
                    current,
                    next,
                    &mut contour_points,
                    &mut contour_edges,
                )?;
                current = next;
                any_edge = true;
            }
            PathSegment::HorizontalLineTo { abs, x } => {
                let next = Point2::new(if abs { x } else { current.x + x }, current.y);
                add_path_line(
                    parser,
                    context,
                    group,
                    current,
                    next,
                    &mut contour_points,
                    &mut contour_edges,
                )?;
                current = next;
                any_edge = true;
            }
            PathSegment::VerticalLineTo { abs, y } => {
                let next = Point2::new(current.x, if abs { y } else { current.y + y });
                add_path_line(
                    parser,
                    context,
                    group,
                    current,
                    next,
                    &mut contour_points,
                    &mut contour_edges,
                )?;
                current = next;
                any_edge = true;
            }
            PathSegment::ClosePath { .. } => {
                let start = subpath_start.ok_or_else(|| SvgImportError::InvalidAttribute {
                    element: "path".to_owned(),
                    attribute: "d".to_owned(),
                })?;
                if current != start {
                    add_path_line(
                        parser,
                        context,
                        group,
                        current,
                        start,
                        &mut contour_points,
                        &mut contour_edges,
                    )?;
                    any_edge = true;
                }
                if contour_points.last() == contour_points.first() {
                    contour_points.pop();
                }
                parser.add_candidate(
                    SvgBoundaryCandidateKind::ClosedPath,
                    &contour_points,
                    contour_edges.clone(),
                )?;
                current = start;
            }
            _ => unreachable!("unsupported path commands were filtered"),
        }
    }
    if !any_edge {
        parser.warnings.add(SvgWarningKind::EmptyGeometryIgnored)?;
    }
    Ok(())
}

fn add_path_line(
    parser: &mut SvgParser,
    context: &ElementContext,
    group: SvgStyleGroupId,
    start: Point2,
    end: Point2,
    contour_points: &mut Vec<Point2>,
    contour_edges: &mut Vec<usize>,
) -> Result<(), SvgImportError> {
    ensure_finite_point(start, "path")?;
    ensure_finite_point(end, "path")?;
    let start = context.ctm.apply(start)?;
    let end = context.ctm.apply(end)?;
    if contour_points.is_empty() {
        contour_points.push(start);
    }
    contour_points.push(end);
    contour_edges.push(parser.add_edge(start, end, group, "path")?);
    Ok(())
}

fn is_shape_name(name: &str) -> bool {
    matches!(name, "line" | "polyline" | "polygon" | "rect" | "path")
}

fn presentation_declarations(
    attributes: &HashMap<String, String>,
) -> Result<StyleDeclarations, SvgImportError> {
    let value = |name: &str| {
        attributes
            .get(name)
            .map(|value| checked_style_value(name, value))
            .transpose()
            .map(|value| value.map(StyleDeclaration::normal))
    };
    Ok(StyleDeclarations {
        stroke: value("stroke")?,
        color: value("color")?,
        fill: value("fill")?,
        stroke_width: value("stroke-width")?,
        stroke_dasharray: value("stroke-dasharray")?,
        stroke_opacity: value("stroke-opacity")?,
        opacity: value("opacity")?,
        display: value("display")?,
        visibility: value("visibility")?,
    })
}

#[derive(Clone, Copy)]
enum DeclarationOrigin {
    Inline,
    Stylesheet,
}

fn parse_declarations(
    text: &str,
    warnings: &mut WarningCollector,
    _origin: DeclarationOrigin,
) -> Result<StyleDeclarations, SvgImportError> {
    if text.len() > MAX_STYLE_TEXT_BYTES {
        return Err(SvgImportError::InvalidCss);
    }
    let mut declarations = StyleDeclarations::default();
    for declaration in text.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let (property, value) = declaration
            .split_once(':')
            .ok_or(SvgImportError::InvalidCss)?;
        let property = property.trim();
        if property.is_empty() {
            return Err(SvgImportError::InvalidCss);
        }
        if !is_supported_style_property(property) {
            warnings.add(SvgWarningKind::UnsupportedStyleProperty(bounded_detail(
                property,
            )))?;
            continue;
        }
        let (value, important) = parse_css_declaration_value(value)?;
        let declaration = StyleDeclaration {
            value: checked_style_value(property, value)?,
            important,
        };
        match property {
            "stroke" => cascade_style_declaration(&mut declarations.stroke, declaration),
            "color" => cascade_style_declaration(&mut declarations.color, declaration),
            "fill" => cascade_style_declaration(&mut declarations.fill, declaration),
            "stroke-width" => {
                cascade_style_declaration(&mut declarations.stroke_width, declaration)
            }
            "stroke-dasharray" => {
                cascade_style_declaration(&mut declarations.stroke_dasharray, declaration)
            }
            "stroke-opacity" => {
                cascade_style_declaration(&mut declarations.stroke_opacity, declaration)
            }
            "opacity" => cascade_style_declaration(&mut declarations.opacity, declaration),
            "display" => cascade_style_declaration(&mut declarations.display, declaration),
            "visibility" => cascade_style_declaration(&mut declarations.visibility, declaration),
            _ => unreachable!("supported style property was checked above"),
        }
    }
    Ok(declarations)
}

fn is_supported_style_property(property: &str) -> bool {
    matches!(
        property,
        "stroke"
            | "color"
            | "fill"
            | "stroke-width"
            | "stroke-dasharray"
            | "stroke-opacity"
            | "opacity"
            | "display"
            | "visibility"
    )
}

fn parse_css_declaration_value(value: &str) -> Result<(&str, bool), SvgImportError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SvgImportError::InvalidCss);
    }
    let Some(marker_start) = value.rfind('!') else {
        return Ok((value, false));
    };
    let marker = value[marker_start + 1..].trim();
    if !marker.eq_ignore_ascii_case("important") {
        return Err(SvgImportError::InvalidCss);
    }
    let value = value[..marker_start].trim();
    if value.is_empty() || value.contains('!') {
        return Err(SvgImportError::InvalidCss);
    }
    Ok((value, true))
}

fn checked_style_value(property: &str, value: &str) -> Result<String, SvgImportError> {
    let value = value.trim();
    if value.chars().take(MAX_STYLE_VALUE_CHARS + 1).count() > MAX_STYLE_VALUE_CHARS {
        return Err(SvgImportError::StyleValueTooLong {
            property: bounded_detail(property),
            maximum: MAX_STYLE_VALUE_CHARS,
        });
    }
    Ok(value.to_owned())
}

fn parse_css_text(
    text: &str,
    limits: SvgImportLimits,
    warnings: &mut WarningCollector,
    rules: &mut Vec<CssRule>,
) -> Result<(), SvgImportError> {
    let text = strip_css_comments(text)?;
    let mut rest = text.as_str();
    while !rest.trim().is_empty() {
        let Some(open) = rest.find('{') else {
            return Err(SvgImportError::InvalidCss);
        };
        let Some(close_relative) = rest[open + 1..].find('}') else {
            return Err(SvgImportError::InvalidCss);
        };
        let close = open + 1 + close_relative;
        let selector_text = rest[..open].trim();
        let declaration_text = &rest[open + 1..close];
        let declarations =
            parse_declarations(declaration_text, warnings, DeclarationOrigin::Stylesheet)?;
        for selector in selector_text.split(',').map(str::trim) {
            if selector.chars().take(MAX_CSS_SELECTOR_CHARS + 1).count() > MAX_CSS_SELECTOR_CHARS {
                return Err(SvgImportError::CssSelectorTooLong {
                    maximum: MAX_CSS_SELECTOR_CHARS,
                });
            }
            let valid = selector
                .strip_prefix('.')
                .filter(|class| is_class_token(class));
            let Some(class) = valid else {
                warnings.add(SvgWarningKind::UnsupportedCssSelector(bounded_detail(
                    selector,
                )))?;
                continue;
            };
            if rules.len() == limits.max_css_rules {
                return Err(SvgImportError::TooManyCssRules {
                    maximum: limits.max_css_rules,
                });
            }
            rules.push(CssRule {
                class: class.to_owned(),
                declarations: declarations.clone(),
            });
        }
        rest = &rest[close + 1..];
    }
    Ok(())
}

fn strip_css_comments(text: &str) -> Result<String, SvgImportError> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(start) = rest.find("/*") else {
            output.push_str(rest);
            return Ok(output);
        };
        output.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("*/") else {
            return Err(SvgImportError::InvalidCss);
        };
        rest = &after[end + 2..];
    }
}

fn parse_classes(value: Option<&str>) -> Result<Vec<String>, SvgImportError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let classes = value
        .split_ascii_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if classes.len() > MAX_CLASS_TOKENS || classes.iter().any(|class| !is_class_token(class)) {
        return Err(SvgImportError::InvalidAttribute {
            element: "element".to_owned(),
            attribute: "class".to_owned(),
        });
    }
    Ok(classes)
}

fn is_class_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn semantic_hint(value: Option<&str>) -> Result<Option<String>, SvgImportError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > MAX_HINT_CHARS
        || value.chars().any(char::is_control)
    {
        return Err(SvgImportError::InvalidAttribute {
            element: "element".to_owned(),
            attribute: "semantic hint".to_owned(),
        });
    }
    Ok(Some(value.to_owned()))
}

fn canonical_origami_kind(value: &str) -> Option<&'static str> {
    match value.trim() {
        "boundary" => Some("boundary"),
        "mountain" => Some("mountain"),
        "valley" => Some("valley"),
        "auxiliary" => Some("auxiliary"),
        "cut" => Some("cut"),
        "ignore" => Some("ignore"),
        _ => None,
    }
}

fn representative_id_hint(value: Option<&str>) -> Option<String> {
    let value = bounded_detail(value?.trim()).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn append_layer_path(
    parent: Option<&str>,
    local: Option<&str>,
) -> Result<Option<String>, SvgImportError> {
    let path = match (parent, local) {
        (None, None) => return Ok(None),
        (Some(parent), None) => parent.to_owned(),
        (None, Some(local)) => local.to_owned(),
        (Some(parent), Some(local)) => format!("{parent} / {local}"),
    };
    if path.chars().count() > MAX_HINT_CHARS {
        return Err(SvgImportError::InvalidAttribute {
            element: "g".to_owned(),
            attribute: "layer path".to_owned(),
        });
    }
    Ok(Some(path))
}

fn parse_opacity(value: &str) -> Result<f64, SvgImportError> {
    let value = value
        .trim()
        .parse::<f64>()
        .map_err(|_| SvgImportError::InvalidCss)?;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(SvgImportError::InvalidCss);
    }
    Ok(value)
}

fn parse_dash_pattern(value: &str) -> Result<SvgDashPattern, ()> {
    if value.trim() == "none" {
        return Ok(SvgDashPattern::Solid);
    }
    if value.trim_end().ends_with(',') {
        return Err(());
    }
    let mut values = Vec::new();
    for length in LengthListParser::from(value) {
        let length = length.map_err(|_| ())?;
        let value = length_to_user_units(length).ok_or(())?;
        if !value.is_finite() || value < 0.0 {
            return Err(());
        }
        values.push(value);
        if values.len() > MAX_DASH_ITEMS {
            return Err(());
        }
    }
    if values.is_empty() || values.iter().all(|value| *value == 0.0) {
        return Ok(SvgDashPattern::Solid);
    }
    if values.len() % 2 == 1 {
        let duplicate = values.clone();
        values.extend(duplicate);
    }
    Ok(SvgDashPattern::Dashes(values))
}

fn shape_length(
    attributes: &HashMap<String, String>,
    attribute: &str,
    default: f64,
    element: &str,
) -> Result<f64, SvgImportError> {
    let Some(value) = attributes.get(attribute) else {
        return Ok(default);
    };
    parse_supported_length(value).map_err(|_| SvgImportError::InvalidAttribute {
        element: element.to_owned(),
        attribute: attribute.to_owned(),
    })
}

fn parse_supported_length(value: &str) -> Result<f64, ()> {
    if let Some(number) = parse_q_length(value) {
        return Ok(number * 96.0 / 101.6);
    }
    let length = Length::from_str(value.trim()).map_err(|_| ())?;
    length_to_user_units(length).ok_or(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RootPhysicalLength {
    millimetres: Option<f64>,
    unit: SvgRootLengthUnit,
    css_pixels_assumed: bool,
}

fn parse_root_physical_length(value: &str) -> Result<RootPhysicalLength, SvgImportError> {
    if let Some(number) = parse_q_length(value) {
        if !number.is_finite() || number <= 0.0 {
            return Err(SvgImportError::InvalidAttribute {
                element: "svg".to_owned(),
                attribute: "width/height".to_owned(),
            });
        }
        return Ok(RootPhysicalLength {
            millimetres: Some(number / 4.0),
            unit: SvgRootLengthUnit::Q,
            css_pixels_assumed: false,
        });
    }
    let length = Length::from_str(value.trim()).map_err(|_| SvgImportError::InvalidAttribute {
        element: "svg".to_owned(),
        attribute: "width/height".to_owned(),
    })?;
    if !length.number.is_finite() || length.number <= 0.0 {
        return Err(SvgImportError::InvalidAttribute {
            element: "svg".to_owned(),
            attribute: "width/height".to_owned(),
        });
    }
    let (millimetres, unit, css_pixels_assumed) = match length.unit {
        LengthUnit::None => (
            Some(length.number * 25.4 / 96.0),
            SvgRootLengthUnit::Unitless,
            true,
        ),
        LengthUnit::Px => (
            Some(length.number * 25.4 / 96.0),
            SvgRootLengthUnit::Px,
            true,
        ),
        LengthUnit::In => (Some(length.number * 25.4), SvgRootLengthUnit::In, false),
        LengthUnit::Cm => (Some(length.number * 10.0), SvgRootLengthUnit::Cm, false),
        LengthUnit::Mm => (Some(length.number), SvgRootLengthUnit::Mm, false),
        LengthUnit::Pt => (
            Some(length.number * 25.4 / 72.0),
            SvgRootLengthUnit::Pt,
            false,
        ),
        LengthUnit::Pc => (
            Some(length.number * 25.4 / 6.0),
            SvgRootLengthUnit::Pc,
            false,
        ),
        LengthUnit::Em => (None, SvgRootLengthUnit::Em, false),
        LengthUnit::Ex => (None, SvgRootLengthUnit::Ex, false),
        LengthUnit::Percent => (None, SvgRootLengthUnit::Percent, false),
    };
    if millimetres.is_some_and(|millimetres| !millimetres.is_finite()) {
        return Err(SvgImportError::InvalidAttribute {
            element: "svg".to_owned(),
            attribute: "width/height".to_owned(),
        });
    }
    Ok(RootPhysicalLength {
        millimetres,
        unit,
        css_pixels_assumed,
    })
}

fn parse_q_length(value: &str) -> Option<f64> {
    let value = value.trim();
    let number = value
        .strip_suffix('Q')
        .or_else(|| value.strip_suffix('q'))?;
    number
        .parse()
        .ok()
        .filter(|number: &f64| number.is_finite())
}

fn parse_view_box(value: &str) -> Result<ViewBox, ()> {
    let value = value.trim();
    if value.is_empty() || value.ends_with(',') {
        return Err(());
    }
    let numbers = NumberListParser::from(value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())?;
    let [x, y, width, height] = numbers.as_slice() else {
        return Err(());
    };
    if ![*x, *y, *width, *height].into_iter().all(f64::is_finite) || *width <= 0.0 || *height <= 0.0
    {
        return Err(());
    }
    Ok(ViewBox::new(*x, *y, *width, *height))
}

fn parse_aspect_ratio(value: &str) -> Result<AspectRatio, ()> {
    let mut tokens = value.split_ascii_whitespace();
    let first = tokens.next().ok_or(())?;
    let (defer, align_token) = if first == "defer" {
        (true, tokens.next().ok_or(())?)
    } else {
        (false, first)
    };
    let align = match align_token {
        "none" => Align::None,
        "xMinYMin" => Align::XMinYMin,
        "xMidYMin" => Align::XMidYMin,
        "xMaxYMin" => Align::XMaxYMin,
        "xMinYMid" => Align::XMinYMid,
        "xMidYMid" => Align::XMidYMid,
        "xMaxYMid" => Align::XMaxYMid,
        "xMinYMax" => Align::XMinYMax,
        "xMidYMax" => Align::XMidYMax,
        "xMaxYMax" => Align::XMaxYMax,
        _ => return Err(()),
    };
    let slice = match tokens.next() {
        None | Some("meet") => false,
        Some("slice") => true,
        Some(_) => return Err(()),
    };
    if tokens.next().is_some() {
        return Err(());
    }
    Ok(AspectRatio {
        defer,
        align,
        slice,
    })
}

fn length_to_user_units(length: Length) -> Option<f64> {
    let value = match length.unit {
        LengthUnit::None | LengthUnit::Px => length.number,
        LengthUnit::In => length.number * 96.0,
        LengthUnit::Cm => length.number * 96.0 / 2.54,
        LengthUnit::Mm => length.number * 96.0 / 25.4,
        LengthUnit::Pt => length.number * 96.0 / 72.0,
        LengthUnit::Pc => length.number * 16.0,
        LengthUnit::Em | LengthUnit::Ex | LengthUnit::Percent => return None,
    };
    value.is_finite().then_some(value)
}

fn parse_transform(value: &str, parser: &mut SvgParser) -> Result<Affine, SvgImportError> {
    let mut transform = Affine::IDENTITY;
    for token in TransformListParser::from(value) {
        parser.transform_count = parser.transform_count.saturating_add(1);
        if parser.transform_count > MAX_TRANSFORM_FUNCTIONS {
            return Err(SvgImportError::TooManyTransforms {
                maximum: MAX_TRANSFORM_FUNCTIONS,
            });
        }
        let token = token.map_err(|_| SvgImportError::InvalidTransform)?;
        let matrix = match token {
            TransformListToken::Matrix { a, b, c, d, e, f } => Affine { a, b, c, d, e, f },
            TransformListToken::Translate { tx, ty } => Affine {
                e: tx,
                f: ty,
                ..Affine::IDENTITY
            },
            TransformListToken::Scale { sx, sy } => Affine {
                a: sx,
                d: sy,
                ..Affine::IDENTITY
            },
            TransformListToken::Rotate { angle } => {
                let radians = angle.to_radians();
                let cosine = radians.cos();
                let sine = radians.sin();
                Affine {
                    a: cosine,
                    b: sine,
                    c: -sine,
                    d: cosine,
                    e: 0.0,
                    f: 0.0,
                }
            }
            TransformListToken::SkewX { angle } => Affine {
                c: angle.to_radians().tan(),
                ..Affine::IDENTITY
            },
            TransformListToken::SkewY { angle } => Affine {
                b: angle.to_radians().tan(),
                ..Affine::IDENTITY
            },
        };
        transform = transform.multiply(matrix);
        if !transform.is_finite_and_invertible() {
            return Err(SvgImportError::InvalidTransform);
        }
    }
    Ok(transform)
}

fn warn_unknown_attributes(
    parser: &mut SvgParser,
    element: &str,
    attributes: &HashMap<String, String>,
) -> Result<(), SvgImportError> {
    for attribute in attributes.keys() {
        let known_global = matches!(
            attribute.as_str(),
            "xmlns"
                | "xmlns:xlink"
                | "xmlns:inkscape"
                | "id"
                | "class"
                | "style"
                | "transform"
                | "stroke"
                | "color"
                | "fill"
                | "stroke-width"
                | "stroke-dasharray"
                | "stroke-opacity"
                | "opacity"
                | "display"
                | "visibility"
                | "inkscape:label"
                | "data-origami-layer"
                | "data-layer"
                | "data-origami-kind"
                | "href"
                | "xlink:href"
                | "xml:base"
        ) || attribute.starts_with("xmlns:");
        let known_geometry = match element {
            "svg" => matches!(
                attribute.as_str(),
                "viewBox" | "width" | "height" | "preserveAspectRatio" | "version"
            ),
            "line" => matches!(attribute.as_str(), "x1" | "y1" | "x2" | "y2"),
            "polyline" | "polygon" => attribute == "points",
            "rect" => matches!(
                attribute.as_str(),
                "x" | "y" | "width" | "height" | "rx" | "ry"
            ),
            "path" => attribute == "d",
            _ => false,
        };
        if !known_global && !known_geometry && !attribute.starts_with("on") {
            parser
                .warnings
                .add(SvgWarningKind::UnsupportedAttribute(bounded_detail(
                    attribute,
                )))?;
        }
    }
    Ok(())
}

fn validate_xml_declaration(declaration: &BytesDecl<'_>) -> Result<(), SvgImportError> {
    let version = declaration
        .version()
        .map_err(|_| SvgImportError::UnsupportedXmlDeclaration)?;
    if version.as_ref() != b"1.0" {
        return Err(SvgImportError::UnsupportedXmlDeclaration);
    }
    if let Some(encoding) = declaration.encoding() {
        let encoding = encoding.map_err(|_| SvgImportError::UnsupportedXmlDeclaration)?;
        if !encoding.as_ref().eq_ignore_ascii_case(b"utf-8") {
            return Err(SvgImportError::UnsupportedXmlDeclaration);
        }
    }
    Ok(())
}

fn append_xml_reference(
    target: &mut String,
    reference: &quick_xml::events::BytesRef<'_>,
) -> Result<(), SvgImportError> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|_| SvgImportError::DoctypeNotAllowed)?
    {
        target.push(character);
        return Ok(());
    }
    let name = reference.decode().map_err(|_| SvgImportError::NonUtf8)?;
    let character = match name.as_ref() {
        "lt" => '<',
        "gt" => '>',
        "amp" => '&',
        "apos" => '\'',
        "quot" => '"',
        _ => return Err(SvgImportError::DoctypeNotAllowed),
    };
    target.push(character);
    Ok(())
}

fn validate_xml_reference(
    reference: &quick_xml::events::BytesRef<'_>,
) -> Result<(), SvgImportError> {
    let mut ignored = String::new();
    append_xml_reference(&mut ignored, reference)
}

fn ensure_finite_point(point: Point2, element: &str) -> Result<(), SvgImportError> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(SvgImportError::NonFiniteNumber {
            element: element.to_owned(),
        })
    }
}

fn is_simple_nonzero_polygon(points: &[Point2]) -> bool {
    if !matches!(
        exact_polygon_orientation(points),
        Ok(Orientation::Clockwise | Orientation::CounterClockwise)
    ) {
        return false;
    }
    for first in 0..points.len() {
        let a = points[first];
        let b = points[(first + 1) % points.len()];
        for second in (first + 1)..points.len() {
            let adjacent =
                first.abs_diff(second) == 1 || (first == 0 && second == points.len() - 1);
            if adjacent {
                continue;
            }
            let c = points[second];
            let d = points[(second + 1) % points.len()];
            if !matches!(
                segment_intersection(a, b, c, d),
                Ok(SegmentIntersection::None)
            ) {
                return false;
            }
        }
    }
    true
}

fn bounded_detail(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(MAX_HINT_CHARS)
        .collect()
}

fn canonical_f64_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

fn position_key(point: Point2) -> (u64, u64) {
    (canonical_f64_bits(point.x), canonical_f64_bits(point.y))
}

#[derive(Debug, Clone, Copy)]
struct WorkingSegment {
    source_index: Option<usize>,
    group: Option<SvgStyleGroupId>,
    kind: EdgeKind,
    start: Point2,
    end: Point2,
}

#[derive(Debug, Clone, Copy)]
struct PlanarSegment {
    group: Option<SvgStyleGroupId>,
    kind: EdgeKind,
    start: Point2,
    end: Point2,
}

fn convert_preview(
    preview: &SvgPreview,
    options: &SvgConversionOptions,
) -> Result<SvgCreasePatternConversion, SvgConversionError> {
    let scale = options.millimetres_per_unit;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(SvgConversionError::InvalidMillimetresPerUnit);
    }

    let mut mappings = HashMap::with_capacity(options.group_mappings.len());
    for mapping in &options.group_mappings {
        if usize::from(mapping.group.0) >= preview.style_groups.len() {
            return Err(SvgConversionError::UnknownGroupMapping {
                group: mapping.group,
            });
        }
        if mappings.insert(mapping.group, mapping.target).is_some() {
            return Err(SvgConversionError::DuplicateGroupMapping {
                group: mapping.group,
            });
        }
    }
    for group in &preview.style_groups {
        if !mappings.contains_key(&group.id) {
            return Err(SvgConversionError::MissingGroupMapping { group: group.id });
        }
    }

    let candidate = options
        .boundary_candidate
        .map(|candidate_id| {
            preview
                .boundary_candidates
                .iter()
                .find(|candidate| candidate.id == candidate_id)
                .ok_or(SvgConversionError::UnknownBoundaryCandidate {
                    candidate: candidate_id,
                })
        })
        .transpose()?;
    if candidate.is_some()
        && mappings
            .values()
            .any(|target| *target == SvgGroupTarget::Boundary)
    {
        return Err(SvgConversionError::BoundaryCandidateMappingConflict);
    }

    let overridden_sources = candidate
        .map(|candidate| {
            candidate
                .source_edge_indices
                .iter()
                .copied()
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let mut retained_kinds = Vec::with_capacity(preview.edges.len());
    let mut used_vertices = vec![false; preview.vertices.len()];
    for edge in &preview.edges {
        let kind = if overridden_sources.contains(&edge.index) {
            Some(EdgeKind::Boundary)
        } else {
            mappings[&edge.style_group].edge_kind()
        };
        if kind.is_some() {
            used_vertices[edge.vertices[0]] = true;
            used_vertices[edge.vertices[1]] = true;
        }
        retained_kinds.push(kind);
    }
    if let Some(candidate) = candidate {
        for vertex in &candidate.vertex_indices {
            used_vertices[*vertex] = true;
        }
    }

    let mut scaled_vertices = vec![None; preview.vertices.len()];
    let mut scaled_positions = HashMap::with_capacity(preview.vertices.len());
    for vertex in preview
        .vertices
        .iter()
        .filter(|vertex| used_vertices[vertex.index])
    {
        let position = Point2::new(vertex.position.x * scale, vertex.position.y * scale);
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(SvgConversionError::ScaledCoordinateNotFinite);
        }
        if scaled_positions
            .insert(position_key(position), vertex.index)
            .is_some()
        {
            return Err(SvgConversionError::ScaledVertexCollision);
        }
        scaled_vertices[vertex.index] = Some(position);
    }

    let mut working = Vec::with_capacity(preview.edges.len().saturating_add(4));
    for (edge, kind) in preview.edges.iter().zip(retained_kinds) {
        let Some(kind) = kind else {
            continue;
        };
        working.push(WorkingSegment {
            source_index: Some(edge.index),
            group: Some(edge.style_group),
            kind,
            start: scaled_vertices[edge.vertices[0]].expect("retained edge vertices were scaled"),
            end: scaled_vertices[edge.vertices[1]].expect("retained edge vertices were scaled"),
        });
    }
    if let Some(candidate) = candidate
        && candidate.kind == SvgBoundaryCandidateKind::ViewBox
    {
        for index in 0..candidate.vertex_indices.len() {
            working.push(WorkingSegment {
                source_index: None,
                group: None,
                kind: EdgeKind::Boundary,
                start: scaled_vertices[candidate.vertex_indices[index]]
                    .expect("selected boundary vertices were scaled"),
                end: scaled_vertices
                    [candidate.vertex_indices[(index + 1) % candidate.vertex_indices.len()]]
                .expect("selected boundary vertices were scaled"),
            });
        }
    }

    let planar = split_intersections(
        &working,
        preview.limits.max_intersection_candidates,
        preview.limits.max_final_edges,
    )?;
    let (positions, indexed_segments) =
        index_planar_segments(&planar, preview.limits.max_final_vertices)?;
    let boundary_indices = boundary_cycle(&indexed_segments, &positions)?;

    let vertex_ids = positions
        .iter()
        .map(|_| VertexId::new())
        .collect::<Vec<_>>();
    let vertices = positions
        .into_iter()
        .zip(vertex_ids.iter().copied())
        .map(|(position, id)| Vertex { id, position })
        .collect::<Vec<_>>();
    let mut group_edge_ids = preview
        .style_groups
        .iter()
        .map(|group| (group.id, Vec::new()))
        .collect::<HashMap<_, _>>();
    let mut edges = Vec::with_capacity(indexed_segments.len());
    let mut has_cuts = false;
    for segment in indexed_segments {
        let id = EdgeId::new();
        if let Some(group) = segment.group {
            group_edge_ids
                .get_mut(&group)
                .expect("converted group originates in the preview")
                .push(id);
        }
        has_cuts |= segment.kind == EdgeKind::Cut;
        edges.push(Edge {
            id,
            start: vertex_ids[segment.vertices[0]],
            end: vertex_ids[segment.vertices[1]],
            kind: segment.kind,
        });
    }
    let boundary_vertices = boundary_indices
        .into_iter()
        .map(|index| vertex_ids[index])
        .collect();
    let groups = preview
        .style_groups
        .iter()
        .map(|group| SvgConvertedGroup {
            group: group.id,
            target: mappings[&group.id],
            edge_ids: group_edge_ids.remove(&group.id).unwrap_or_default(),
        })
        .collect();

    Ok(SvgCreasePatternConversion {
        crease_pattern: CreasePattern { vertices, edges },
        boundary_vertices,
        groups,
        has_cuts,
    })
}

fn split_intersections(
    segments: &[WorkingSegment],
    maximum_candidates: usize,
    maximum_edges: usize,
) -> Result<Vec<PlanarSegment>, SvgConversionError> {
    let mut by_min_x = (0..segments.len()).collect::<Vec<_>>();
    by_min_x.sort_unstable_by(|left, right| {
        let left_min = segments[*left].start.x.min(segments[*left].end.x);
        let right_min = segments[*right].start.x.min(segments[*right].end.x);
        left_min.total_cmp(&right_min).then_with(|| left.cmp(right))
    });
    let mut split_points = segments
        .iter()
        .map(|segment| vec![segment.start, segment.end])
        .collect::<Vec<_>>();
    let mut candidates = 0_usize;
    for (position, left_index) in by_min_x.iter().copied().enumerate() {
        let left = segments[left_index];
        let left_max_x = left.start.x.max(left.end.x);
        let left_min_y = left.start.y.min(left.end.y);
        let left_max_y = left.start.y.max(left.end.y);
        for right_index in by_min_x.iter().copied().skip(position + 1) {
            let right = segments[right_index];
            if right.start.x.min(right.end.x) > left_max_x {
                break;
            }
            if left_min_y > right.start.y.max(right.end.y)
                || right.start.y.min(right.end.y) > left_max_y
            {
                continue;
            }
            candidates = candidates.saturating_add(1);
            if candidates > maximum_candidates {
                return Err(SvgConversionError::TooManyIntersectionCandidates {
                    maximum: maximum_candidates,
                });
            }
            match segment_intersection(left.start, left.end, right.start, right.end) {
                Ok(SegmentIntersection::None) => {}
                Ok(SegmentIntersection::Point(point)) => {
                    split_points[left_index].push(point);
                    split_points[right_index].push(point);
                }
                Ok(SegmentIntersection::CollinearOverlap) => {
                    return Err(SvgConversionError::CollinearOverlap {
                        first: left.source_index.unwrap_or(left_index),
                        second: right.source_index.unwrap_or(right_index),
                    });
                }
                Err(_) => return Err(SvgConversionError::IntersectionNotRepresentable),
            }
        }
    }

    let mut output = Vec::new();
    for (segment, mut points) in segments.iter().copied().zip(split_points) {
        sort_along_segment(&mut points, segment.start, segment.end);
        points.dedup_by_key(|point| position_key(*point));
        if points.len() < 2 {
            return Err(SvgConversionError::InvalidFinalEdge);
        }
        for pair in points.windows(2) {
            if pair[0] == pair[1] {
                return Err(SvgConversionError::InvalidFinalEdge);
            }
            if output.len() == maximum_edges {
                return Err(SvgConversionError::TooManyFinalEdges {
                    maximum: maximum_edges,
                });
            }
            output.push(PlanarSegment {
                group: segment.group,
                kind: segment.kind,
                start: pair[0],
                end: pair[1],
            });
        }
    }
    Ok(output)
}

fn sort_along_segment(points: &mut [Point2], start: Point2, end: Point2) {
    let use_x = (end.x - start.x).abs() >= (end.y - start.y).abs();
    let ascending = if use_x {
        end.x >= start.x
    } else {
        end.y >= start.y
    };
    points.sort_unstable_by(|left, right| {
        let order = if use_x {
            left.x
                .total_cmp(&right.x)
                .then_with(|| left.y.total_cmp(&right.y))
        } else {
            left.y
                .total_cmp(&right.y)
                .then_with(|| left.x.total_cmp(&right.x))
        };
        if ascending { order } else { order.reverse() }
    });
}

#[derive(Debug, Clone, Copy)]
struct IndexedSegment {
    group: Option<SvgStyleGroupId>,
    kind: EdgeKind,
    vertices: [usize; 2],
}

fn index_planar_segments(
    segments: &[PlanarSegment],
    maximum_vertices: usize,
) -> Result<(Vec<Point2>, Vec<IndexedSegment>), SvgConversionError> {
    let mut positions = Vec::new();
    let mut position_indices = HashMap::new();
    let mut edge_keys = HashSet::new();
    let mut indexed = Vec::with_capacity(segments.len());
    for segment in segments {
        let mut endpoints = [0_usize; 2];
        for (slot, point) in endpoints.iter_mut().zip([segment.start, segment.end]) {
            let key = position_key(point);
            *slot = if let Some(index) = position_indices.get(&key) {
                *index
            } else {
                if positions.len() == maximum_vertices {
                    return Err(SvgConversionError::TooManyFinalVertices {
                        maximum: maximum_vertices,
                    });
                }
                let index = positions.len();
                positions.push(point);
                position_indices.insert(key, index);
                index
            };
        }
        if endpoints[0] == endpoints[1] {
            return Err(SvgConversionError::InvalidFinalEdge);
        }
        let key = if endpoints[0] < endpoints[1] {
            (endpoints[0], endpoints[1])
        } else {
            (endpoints[1], endpoints[0])
        };
        if !edge_keys.insert(key) {
            return Err(SvgConversionError::InvalidFinalEdge);
        }
        indexed.push(IndexedSegment {
            group: segment.group,
            kind: segment.kind,
            vertices: endpoints,
        });
    }
    Ok((positions, indexed))
}

fn boundary_cycle(
    segments: &[IndexedSegment],
    positions: &[Point2],
) -> Result<Vec<usize>, SvgConversionError> {
    let boundary = segments
        .iter()
        .filter(|segment| segment.kind == EdgeKind::Boundary)
        .collect::<Vec<_>>();
    if boundary.len() > MAX_BOUNDARY_EDGES {
        return Err(SvgConversionError::TooManyBoundaryEdges {
            maximum: MAX_BOUNDARY_EDGES,
        });
    }
    if boundary.len() < 3 {
        return Err(SvgConversionError::BoundaryTooSmall {
            actual: boundary.len(),
        });
    }
    let mut adjacency = vec![Vec::new(); positions.len()];
    for segment in &boundary {
        adjacency[segment.vertices[0]].push(segment.vertices[1]);
        adjacency[segment.vertices[1]].push(segment.vertices[0]);
    }
    for neighbours in &adjacency {
        if !neighbours.is_empty() && neighbours.len() != 2 {
            return Err(SvgConversionError::BoundaryDegree {
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
            if current != start || cycle.len() != boundary.len() {
                return Err(SvgConversionError::BoundaryDisconnected);
            }
            validate_boundary_geometry(&cycle, positions)?;
            return Ok(cycle);
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
    positions: &[Point2],
) -> Result<(), SvgConversionError> {
    let polygon = cycle
        .iter()
        .map(|index| positions[*index])
        .collect::<Vec<_>>();
    match exact_polygon_orientation(&polygon)
        .map_err(|_| SvgConversionError::BoundaryGeometryUnrepresentable)?
    {
        Orientation::Collinear => return Err(SvgConversionError::BoundaryZeroArea),
        Orientation::Clockwise | Orientation::CounterClockwise => {}
    }
    for first in 0..polygon.len() {
        for second in (first + 1)..polygon.len() {
            let adjacent =
                first.abs_diff(second) == 1 || (first == 0 && second == polygon.len() - 1);
            if adjacent {
                continue;
            }
            match segment_intersection(
                polygon[first],
                polygon[(first + 1) % polygon.len()],
                polygon[second],
                polygon[(second + 1) % polygon.len()],
            ) {
                Ok(SegmentIntersection::None) => {}
                Ok(_) => return Err(SvgConversionError::BoundarySelfIntersection),
                Err(_) => return Err(SvgConversionError::BoundaryGeometryUnrepresentable),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ori_geometry::validate_crease_pattern;

    use super::*;

    fn document(root_attributes: &str, body: &str) -> String {
        format!(
            r##"<svg xmlns="{SVG_NAMESPACE}" {root_attributes} stroke="#000" fill="none">{body}</svg>"##
        )
    }

    fn standard_document(body: &str) -> String {
        document(
            r#"viewBox="0 0 100 100" width="100mm" height="100mm""#,
            body,
        )
    }

    fn preview(body: &str) -> SvgPreview {
        let source = standard_document(body);
        read_svg_preview(source.as_bytes()).expect("SVG preview")
    }

    fn mappings(preview: &SvgPreview, target: SvgGroupTarget) -> Vec<SvgGroupMapping> {
        preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target,
            })
            .collect()
    }

    fn conversion_options(
        preview: &SvgPreview,
        target: SvgGroupTarget,
        boundary_candidate: Option<SvgBoundaryCandidateId>,
    ) -> SvgConversionOptions {
        SvgConversionOptions {
            millimetres_per_unit: 1.0,
            group_mappings: mappings(preview, target),
            boundary_candidate,
        }
    }

    fn candidate(preview: &SvgPreview, kind: SvgBoundaryCandidateKind) -> SvgBoundaryCandidateId {
        preview
            .boundary_candidates()
            .iter()
            .find(|candidate| candidate.kind == kind)
            .expect("boundary candidate kind")
            .id
    }

    fn has_warning(preview: &SvgPreview, expected: &SvgWarningKind) -> bool {
        preview
            .warnings()
            .iter()
            .any(|warning| &warning.kind == expected)
    }

    fn assert_approx(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-10,
            "expected {expected}, found {actual}"
        );
    }

    #[test]
    fn imports_supported_straight_geometry_and_closed_candidates() {
        let preview = preview(
            r#"
                <line x1="1" y1="2" x2="3" y2="4"/>
                <polyline points="10,10 20,10 20,20"/>
                <polyline points="30,30 40,30 40,40 30,30"/>
                <polygon points="50,50 60,50 55,60"/>
                <rect x="65" y="65" width="10" height="5"/>
                <path d="M 80 80 90 80 h 5 v 10 H 80 z"/>
            "#,
        );

        assert_eq!(preview.edges().len(), 1 + 2 + 3 + 3 + 4 + 5);
        assert_eq!(preview.style_groups().len(), 1);
        assert_eq!(
            preview.style_groups()[0].segment_count,
            preview.edges().len()
        );
        let kinds = preview
            .boundary_candidates()
            .iter()
            .map(|candidate| candidate.kind)
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                SvgBoundaryCandidateKind::ViewBox,
                SvgBoundaryCandidateKind::Polyline,
                SvgBoundaryCandidateKind::Polygon,
                SvgBoundaryCandidateKind::Rectangle,
                SvgBoundaryCandidateKind::ClosedPath,
            ]
        );
    }

    #[test]
    fn supports_relative_paths_and_implicit_move_lines() {
        let preview = preview(r#"<path d="m 10 10 10 0 v 10 h -10 z"/>"#);
        assert_eq!(preview.edges().len(), 4);
        let positions = preview
            .edges()
            .iter()
            .flat_map(|edge| edge.vertices)
            .map(|index| preview.vertices()[index].position)
            .map(position_key)
            .collect::<HashSet<_>>();
        assert!(positions.contains(&position_key(Point2::new(10.0, 10.0))));
        assert!(positions.contains(&position_key(Point2::new(20.0, 10.0))));
        assert!(positions.contains(&position_key(Point2::new(20.0, 20.0))));
        assert!(positions.contains(&position_key(Point2::new(10.0, 20.0))));
    }

    #[test]
    fn excludes_curves_and_rounded_rectangles_without_approximation() {
        let preview = preview(
            r#"
                <path d="M 0 0 C 1 1 2 2 3 3 L 4 4"/>
                <rect x="10" y="10" width="20" height="20" rx="2"/>
                <line x1="0" y1="10" x2="10" y2="10"/>
            "#,
        );

        assert_eq!(preview.edges().len(), 1);
        assert_eq!(preview.style_groups().len(), 1);
        assert!(has_warning(
            &preview,
            &SvgWarningKind::UnsupportedPathCommand('C')
        ));
        assert!(has_warning(
            &preview,
            &SvgWarningKind::UnsupportedElement("rounded rect".to_owned())
        ));
    }

    #[test]
    fn rejects_empty_and_curve_only_svg_documents() {
        for body in ["", r#"<path d="M 0 0 C 1 1 2 2 3 3"/>"#] {
            let source = standard_document(body);
            assert!(matches!(
                read_svg_preview(source.as_bytes()),
                Err(SvgImportError::NoSupportedGeometry)
            ));
        }
    }

    #[test]
    fn composes_nested_affine_transforms_without_flipping_y() {
        let preview = preview(
            r#"<g transform="translate(10 20)"><g transform="scale(2)">
                <line x1="1" y1="2" x2="3" y2="4"/>
            </g></g>"#,
        );
        let edge = preview.edges()[0];
        assert_eq!(
            preview.vertices()[edge.vertices[0]].position,
            Point2::new(12.0, 24.0)
        );
        assert_eq!(
            preview.vertices()[edge.vertices[1]].position,
            Point2::new(16.0, 28.0)
        );
    }

    #[test]
    fn supports_rotation_about_a_center() {
        let preview =
            preview(r#"<line transform="rotate(90 10 0)" x1="10" y1="0" x2="20" y2="0"/>"#);
        let edge = preview.edges()[0];
        let start = preview.vertices()[edge.vertices[0]].position;
        let end = preview.vertices()[edge.vertices[1]].position;
        assert_approx(start.x, 10.0);
        assert_approx(start.y, 0.0);
        assert_approx(end.x, 10.0);
        assert_approx(end.y, 10.0);
    }

    #[test]
    fn applies_css_inheritance_inline_precedence_and_mapping_hints() {
        let source = document(
            r#"viewBox="0 0 10 10" width="10mm" height="10mm""#,
            r#"
                <style>.fold { stroke: #ff0000; stroke-width: 2; stroke-dasharray: 2 3; }</style>
                <g class="fold" data-origami-layer="creases" data-origami-kind="mountain" opacity="0.5">
                    <line class="chosen" style="stroke: #00ff00" x1="0" y1="0" x2="10" y2="10"/>
                </g>
            "#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("styled SVG");
        let group = &preview.style_groups()[0];

        assert_eq!(
            group.stroke,
            RgbaColor {
                red: 0,
                green: 255,
                blue: 0,
                alpha: 128,
            }
        );
        assert_eq!(group.stroke_width, 2.0);
        assert_eq!(group.dash_pattern, SvgDashPattern::Dashes(vec![2.0, 3.0]));
        assert_eq!(group.classes, vec!["chosen", "fold"]);
        assert_eq!(group.layer.as_deref(), Some("creases"));
        assert_eq!(group.semantic.as_deref(), Some("mountain"));
    }

    #[test]
    fn builds_bounded_layer_paths_without_using_shape_ids_as_layers() {
        let source = document(
            r#"viewBox="0 0 10 10" width="10mm" height="10mm"
               xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape""#,
            r#"
                <g id="outer-id" data-origami-layer="paper" inkscape:label="ignored">
                    <g id="inner-id" inkscape:label="creases" data-layer="ignored">
                        <line id="shape-id" data-origami-layer="shape-ignored"
                              data-fold="mountain" x1="0" y1="0" x2="10" y2="10"/>
                    </g>
                    <g id="identifier-only">
                        <line class="id-fallback" x1="0" y1="10" x2="10" y2="0"/>
                    </g>
                </g>
            "#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("layered SVG");
        let group = &preview.style_groups()[0];

        assert_eq!(group.layer.as_deref(), Some("paper / creases"));
        assert_eq!(group.semantic, None);
        assert!(
            preview
                .style_groups()
                .iter()
                .any(|group| group.layer.as_deref() == Some("paper / identifier-only"))
        );
        assert!(has_warning(
            &preview,
            &SvgWarningKind::UnsupportedAttribute("data-fold".to_owned())
        ));
    }

    #[test]
    fn uses_the_first_retained_shape_id_without_splitting_style_groups() {
        let preview = preview(
            r#"
                <path id="unsupported-first" d="M0 0 C1 1 2 2 3 3"/>
                <line x1="0" y1="20" x2="10" y2="20"/>
                <line id="first-line" x1="0" y1="0" x2="10" y2="0"/>
                <line id="second-line" x1="0" y1="10" x2="10" y2="10"/>
            "#,
        );

        assert_eq!(preview.style_groups().len(), 1);
        assert_eq!(
            preview.style_groups()[0].representative_id.as_deref(),
            Some("first-line")
        );
        assert_eq!(preview.style_groups()[0].segment_count, 3);
        assert_eq!(preview.style_groups()[0].element_count, 3);

        let long_id = format!("\u{0001}{}", "a".repeat(MAX_HINT_CHARS + 20));
        let sanitized = representative_id_hint(Some(&long_id)).expect("sanitized ID");
        assert_eq!(sanitized.chars().count(), MAX_HINT_CHARS);
        assert!(sanitized.chars().all(|character| character == 'a'));
        assert_eq!(representative_id_hint(Some("\n\t")), None);
    }

    #[test]
    fn counts_source_shapes_separately_from_generated_segments() {
        let preview = preview(
            r#"
                <rect x="10" y="10" width="20" height="20"/>
                <polyline points="40,40 50,40 50,50"/>
            "#,
        );
        assert_eq!(preview.style_groups().len(), 1);
        assert_eq!(preview.style_groups()[0].element_count, 2);
        assert_eq!(preview.style_groups()[0].segment_count, 6);
    }

    #[test]
    fn retains_only_canonical_origami_kind_hints() {
        let canonical = [
            "boundary",
            "mountain",
            "valley",
            "auxiliary",
            "cut",
            "ignore",
        ];
        let mut body = String::new();
        for (index, kind) in canonical.iter().enumerate() {
            body.push_str(&format!(
                r#"<line data-origami-kind="{kind}" x1="0" y1="{index}" x2="10" y2="{index}"/>"#
            ));
        }
        body.push_str(
            r#"
                <line data-origami-kind="Mountain" x1="20" y1="0" x2="30" y2="0"/>
                <line data-origami-kind="custom-fold" x1="20" y1="1" x2="30" y2="1"/>
            "#,
        );
        let preview = preview(&body);

        let semantics = preview
            .style_groups()
            .iter()
            .filter_map(|group| group.semantic.clone())
            .collect::<HashSet<_>>();
        assert_eq!(
            semantics,
            canonical.into_iter().map(str::to_owned).collect()
        );
        let unresolved = preview
            .style_groups()
            .iter()
            .find(|group| group.semantic.is_none())
            .expect("non-canonical hints share the no-semantic group");
        assert_eq!(unresolved.element_count, 2);
        let warning = preview
            .warnings()
            .iter()
            .find(|warning| {
                warning.kind == SvgWarningKind::UnsupportedAttribute("data-origami-kind".to_owned())
            })
            .expect("non-canonical semantic warning");
        assert_eq!(warning.occurrences, 2);
    }

    #[test]
    fn resolves_inherited_presentation_class_and_inline_current_color() {
        let styled_preview = preview(
            r##"
                <style>.css-color { color: #445566; stroke: currentColor; }</style>
                <g class="presentation" color="#112233" stroke="currentColor">
                    <line x1="0" y1="10" x2="10" y2="10"/>
                </g>
                <g class="css-color">
                    <line x1="0" y1="20" x2="10" y2="20"/>
                </g>
                <line class="inline" color="#0000ff"
                      style="color: #abcdef; stroke: currentColor"
                      x1="0" y1="30" x2="10" y2="30"/>
            "##,
        );
        let colors = styled_preview
            .style_groups()
            .iter()
            .map(|group| (group.stroke.red, group.stroke.green, group.stroke.blue))
            .collect::<HashSet<_>>();

        assert_eq!(colors.len(), 3);
        assert!(colors.contains(&(0x11, 0x22, 0x33)));
        assert!(colors.contains(&(0x44, 0x55, 0x66)));
        assert!(colors.contains(&(0xab, 0xcd, 0xef)));

        let recursive = standard_document(
            r#"<line stroke="currentColor" color="currentColor" x1="0" y1="0" x2="1" y2="1"/>"#,
        );
        assert!(matches!(
            read_svg_preview(recursive.as_bytes()),
            Err(SvgImportError::NoSupportedGeometry)
        ));
    }

    #[test]
    fn recommends_physical_scale_only_from_valid_root_geometry() {
        let source = document(
            r#"viewBox="0 0 210 297" width="21cm" height="297mm""#,
            r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("physical SVG");
        assert_eq!(preview.recommended_millimetres_per_unit(), Some(1.0));
        assert_eq!(
            preview.root_view_box(),
            Some(SvgRootViewBox {
                x: 0.0,
                y: 0.0,
                width: 210.0,
                height: 297.0,
            })
        );
        assert_eq!(
            preview.root_physical_size(),
            SvgRootPhysicalSize {
                width_millimetres: Some(210.0),
                height_millimetres: Some(297.0),
                width_unit: Some(SvgRootLengthUnit::Cm),
                height_unit: Some(SvgRootLengthUnit::Mm),
            }
        );
        assert!(!has_warning(
            &preview,
            &SvgWarningKind::CssPixelScaleAssumed
        ));

        let source = document(
            r#"viewBox="0 0 100 100" width="100mm" height="200mm""#,
            r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("non-uniform SVG");
        assert_eq!(preview.recommended_millimetres_per_unit(), None);
        assert!(has_warning(
            &preview,
            &SvgWarningKind::PhysicalScaleNeedsSelection
        ));

        let source = document(
            r#"viewBox="0 0 1 1" width="0.000000001mm" height="0.0000000010005mm""#,
            r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("tiny non-uniform SVG");
        assert_eq!(preview.recommended_millimetres_per_unit(), None);

        let source = document(
            r#"viewBox="0 0 1 1" width="1e308mm" height="1e308mm""#,
            r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("huge uniform SVG");
        assert_eq!(preview.recommended_millimetres_per_unit(), Some(1e308));
    }

    #[test]
    fn reports_when_root_dimensions_use_css_pixels() {
        let source = document(
            r#"viewBox="10 20 96 192" width="96px" height="192""#,
            r#"<line x1="10" y1="20" x2="11" y2="21"/>"#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("CSS pixel SVG");
        assert_approx(
            preview
                .recommended_millimetres_per_unit()
                .expect("automatic scale"),
            25.4 / 96.0,
        );
        assert_eq!(
            preview.root_view_box(),
            Some(SvgRootViewBox {
                x: 10.0,
                y: 20.0,
                width: 96.0,
                height: 192.0,
            })
        );
        let physical_size = preview.root_physical_size();
        assert_approx(
            physical_size.width_millimetres.expect("physical width"),
            25.4,
        );
        assert_approx(
            physical_size.height_millimetres.expect("physical height"),
            50.8,
        );
        assert_eq!(physical_size.width_unit, Some(SvgRootLengthUnit::Px));
        assert_eq!(physical_size.height_unit, Some(SvgRootLengthUnit::Unitless));
        assert!(has_warning(&preview, &SvgWarningKind::CssPixelScaleAssumed));

        let source = document(
            r#"viewBox="0 0 96 96" width="96px" height="192px""#,
            r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("non-uniform CSS pixel SVG");
        assert_eq!(preview.recommended_millimetres_per_unit(), None);
        assert!(has_warning(
            &preview,
            &SvgWarningKind::PhysicalScaleNeedsSelection
        ));
        assert!(has_warning(&preview, &SvgWarningKind::CssPixelScaleAssumed));
    }

    #[test]
    fn accepts_trimmed_absolute_lengths_and_q_units() {
        let inch = parse_root_physical_length(" 1in ").unwrap();
        assert_approx(inch.millimetres.unwrap(), 25.4);
        assert_eq!(inch.unit, SvgRootLengthUnit::In);
        assert!(!inch.css_pixels_assumed);
        let q = parse_root_physical_length(" 4Q ").unwrap();
        assert_approx(q.millimetres.unwrap(), 1.0);
        assert_eq!(q.unit, SvgRootLengthUnit::Q);
        assert!(!q.css_pixels_assumed);
        let pixels = parse_root_physical_length(" 96px ").unwrap();
        assert_approx(pixels.millimetres.unwrap(), 25.4);
        assert_eq!(pixels.unit, SvgRootLengthUnit::Px);
        assert!(pixels.css_pixels_assumed);
        let unitless = parse_root_physical_length("96").unwrap();
        assert_eq!(unitless.unit, SvgRootLengthUnit::Unitless);
        assert!(unitless.css_pixels_assumed);
        let relative = parse_root_physical_length("2em").unwrap();
        assert_eq!(relative.millimetres, None);
        assert_eq!(relative.unit, SvgRootLengthUnit::Em);
        assert!(parse_root_physical_length("1e309Q").is_err());
        assert_approx(parse_supported_length(" 25.4mm ").unwrap(), 96.0);
    }

    #[test]
    fn source_candidate_overrides_group_mapping_and_preserves_correspondence() {
        let preview = preview(r#"<rect x="10" y="10" width="80" height="80"/>"#);
        let rectangle = candidate(&preview, SvgBoundaryCandidateKind::Rectangle);
        let converted = preview
            .convert(&conversion_options(
                &preview,
                SvgGroupTarget::Mountain,
                Some(rectangle),
            ))
            .expect("rectangle conversion");

        assert_eq!(converted.boundary_vertices().len(), 4);
        assert_eq!(converted.crease_pattern().edges.len(), 4);
        assert!(
            converted
                .crease_pattern()
                .edges
                .iter()
                .all(|edge| edge.kind == EdgeKind::Boundary)
        );
        assert_eq!(converted.groups()[0].target, SvgGroupTarget::Mountain);
        assert_eq!(converted.groups()[0].edge_ids.len(), 4);
    }

    #[test]
    fn rejects_candidate_and_boundary_group_combination() {
        let preview = preview(r#"<rect x="10" y="10" width="80" height="80"/>"#);
        let rectangle = candidate(&preview, SvgBoundaryCandidateKind::Rectangle);
        let error = preview
            .convert(&conversion_options(
                &preview,
                SvgGroupTarget::Boundary,
                Some(rectangle),
            ))
            .expect_err("candidate conflict");
        assert_eq!(error, SvgConversionError::BoundaryCandidateMappingConflict);
    }

    #[test]
    fn view_box_candidate_planarizes_x_crossings_and_boundary_touches() {
        let preview = preview(
            r#"
                <line x1="0" y1="50" x2="100" y2="50"/>
                <line x1="50" y1="0" x2="50" y2="100"/>
            "#,
        );
        let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
        let converted = preview
            .convert(&conversion_options(
                &preview,
                SvgGroupTarget::Mountain,
                Some(view_box),
            ))
            .expect("planarized crossing");

        assert_eq!(converted.crease_pattern().vertices.len(), 9);
        assert_eq!(converted.crease_pattern().edges.len(), 12);
        assert_eq!(
            converted
                .crease_pattern()
                .edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Boundary)
                .count(),
            8
        );
        assert_eq!(converted.groups()[0].edge_ids.len(), 4);
        assert!(validate_crease_pattern(converted.crease_pattern()).is_valid());
    }

    #[test]
    fn planarizes_t_junctions() {
        let preview = preview(
            r#"
                <line x1="0" y1="50" x2="100" y2="50"/>
                <line x1="50" y1="0" x2="50" y2="50"/>
            "#,
        );
        let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
        let converted = preview
            .convert(&conversion_options(
                &preview,
                SvgGroupTarget::Valley,
                Some(view_box),
            ))
            .expect("T junction");

        assert_eq!(converted.groups()[0].edge_ids.len(), 3);
        assert!(validate_crease_pattern(converted.crease_pattern()).is_valid());
    }

    #[test]
    fn rejects_collinear_overlap_instead_of_guessing() {
        let preview = preview(r#"<line x1="0" y1="0" x2="100" y2="0"/>"#);
        let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
        let error = preview
            .convert(&conversion_options(
                &preview,
                SvgGroupTarget::Auxiliary,
                Some(view_box),
            ))
            .expect_err("overlapping boundary");
        assert!(matches!(error, SvgConversionError::CollinearOverlap { .. }));
    }

    #[test]
    fn maps_source_boundary_without_selecting_a_candidate() {
        let preview = preview(r#"<rect x="10" y="10" width="80" height="80"/>"#);
        let converted = preview
            .convert(&conversion_options(
                &preview,
                SvgGroupTarget::Boundary,
                None,
            ))
            .expect("mapped boundary");
        assert_eq!(converted.boundary_vertices().len(), 4);
        assert!(validate_crease_pattern(converted.crease_pattern()).is_valid());
    }

    #[test]
    fn rejects_disconnected_boundary_cycles() {
        let preview = preview(
            r#"
                <rect x="10" y="10" width="20" height="20"/>
                <rect x="60" y="60" width="20" height="20"/>
            "#,
        );
        let error = preview
            .convert(&conversion_options(
                &preview,
                SvgGroupTarget::Boundary,
                None,
            ))
            .expect_err("disconnected boundaries");
        assert_eq!(error, SvgConversionError::BoundaryDisconnected);
    }

    #[test]
    fn reports_cut_edges_and_ignored_groups() {
        let source = standard_document(
            r#"
                <line class="cut" x1="10" y1="10" x2="90" y2="90"/>
                <line class="guide" x1="10" y1="90" x2="90" y2="10"/>
            "#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("cut preview");
        assert_eq!(preview.style_groups().len(), 2);
        let options = SvgConversionOptions {
            millimetres_per_unit: 1.0,
            group_mappings: vec![
                SvgGroupMapping {
                    group: preview.style_groups()[0].id,
                    target: SvgGroupTarget::Cut,
                },
                SvgGroupMapping {
                    group: preview.style_groups()[1].id,
                    target: SvgGroupTarget::Ignore,
                },
            ],
            boundary_candidate: Some(candidate(&preview, SvgBoundaryCandidateKind::ViewBox)),
        };
        let converted = preview.convert(&options).expect("cut conversion");

        assert!(converted.has_cuts());
        assert_eq!(converted.groups()[0].edge_ids.len(), 1);
        assert!(converted.groups()[1].edge_ids.is_empty());
    }

    #[test]
    fn requires_exactly_one_mapping_for_every_group() {
        let preview = preview(r#"<line x1="10" y1="10" x2="90" y2="90"/>"#);
        let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);

        let missing = SvgConversionOptions {
            millimetres_per_unit: 1.0,
            group_mappings: Vec::new(),
            boundary_candidate: Some(view_box),
        };
        assert_eq!(
            preview.convert(&missing).expect_err("missing mapping"),
            SvgConversionError::MissingGroupMapping {
                group: preview.style_groups()[0].id,
            }
        );

        let group = preview.style_groups()[0].id;
        let duplicate = SvgConversionOptions {
            millimetres_per_unit: 1.0,
            group_mappings: vec![
                SvgGroupMapping {
                    group,
                    target: SvgGroupTarget::Mountain,
                },
                SvgGroupMapping {
                    group,
                    target: SvgGroupTarget::Valley,
                },
            ],
            boundary_candidate: Some(view_box),
        };
        assert_eq!(
            preview.convert(&duplicate).expect_err("duplicate mapping"),
            SvgConversionError::DuplicateGroupMapping { group }
        );

        let unknown_group = SvgStyleGroupId(999);
        let unknown = SvgConversionOptions {
            millimetres_per_unit: 1.0,
            group_mappings: vec![SvgGroupMapping {
                group: unknown_group,
                target: SvgGroupTarget::Mountain,
            }],
            boundary_candidate: Some(view_box),
        };
        assert_eq!(
            preview.convert(&unknown).expect_err("unknown mapping"),
            SvgConversionError::UnknownGroupMapping {
                group: unknown_group,
            }
        );
    }

    #[test]
    fn validates_scale_and_boundary_candidate_ids() {
        let preview = preview(r#"<line x1="10" y1="10" x2="90" y2="90"/>"#);
        let options = SvgConversionOptions {
            millimetres_per_unit: 0.0,
            group_mappings: mappings(&preview, SvgGroupTarget::Mountain),
            boundary_candidate: None,
        };
        assert_eq!(
            preview.convert(&options).expect_err("zero scale"),
            SvgConversionError::InvalidMillimetresPerUnit
        );

        let options = SvgConversionOptions {
            millimetres_per_unit: 1.0,
            group_mappings: mappings(&preview, SvgGroupTarget::Mountain),
            boundary_candidate: Some(SvgBoundaryCandidateId(999)),
        };
        assert_eq!(
            preview.convert(&options).expect_err("unknown candidate"),
            SvgConversionError::UnknownBoundaryCandidate {
                candidate: SvgBoundaryCandidateId(999),
            }
        );
    }

    #[test]
    fn deduplicates_exact_source_endpoints() {
        let source = document(
            "",
            r#"
                <line x1="0" y1="0" x2="10" y2="0"/>
                <line x1="10" y1="0" x2="10" y2="10"/>
            "#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("joined lines");
        assert_eq!(preview.vertices().len(), 3);
        assert_eq!(
            preview.edges()[0].vertices[1],
            preview.edges()[1].vertices[0]
        );
    }

    #[test]
    fn enforces_svg_namespace_for_root_and_descendants() {
        let missing = br##"<svg stroke="#000"><line x2="10"/></svg>"##;
        assert!(matches!(
            read_svg_preview(missing),
            Err(SvgImportError::InvalidSvgNamespace)
        ));

        let source = standard_document(
            r#"
                <g xmlns="urn:not-svg"><line x1="0" y1="0" x2="10" y2="10"/></g>
                <line x1="10" y1="10" x2="20" y2="20"/>
            "#,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("namespace switch");
        assert_eq!(preview.edges().len(), 1);
        assert!(has_warning(
            &preview,
            &SvgWarningKind::UnsupportedElement("g".to_owned())
        ));
    }

    #[test]
    fn ignores_stylesheets_from_other_namespaces() {
        let source = document(
            r#"xmlns:x="urn:not-svg" viewBox="0 0 10 10" width="10mm" height="10mm""#,
            r##"
                <x:style>.fold { stroke: red; }</x:style>
                <line class="fold" stroke="none" x1="0" y1="0" x2="10" y2="10"/>
                <line stroke="#000" x1="0" y1="10" x2="10" y2="0"/>
            "##,
        );
        let preview = read_svg_preview(source.as_bytes()).expect("foreign stylesheet");
        assert_eq!(preview.edges().len(), 1);
    }

    #[test]
    fn rejects_undeclared_prefixes_doctypes_and_non_utf8() {
        let undeclared = br#"<svg xmlns="http://www.w3.org/2000/svg"><x:line x2="1"/></svg>"#;
        assert!(matches!(
            read_svg_preview(undeclared),
            Err(SvgImportError::InvalidXml(_))
        ));

        let doctype =
            br#"<!DOCTYPE svg [<!ENTITY x "1">]><svg xmlns="http://www.w3.org/2000/svg"/>"#;
        assert!(matches!(
            read_svg_preview(doctype),
            Err(SvgImportError::DoctypeNotAllowed)
        ));

        assert!(matches!(
            read_svg_preview(&[0xff, 0xfe, 0x00]),
            Err(SvgImportError::NonUtf8)
        ));
    }

    #[test]
    fn rejects_invalid_xml_declarations_and_trailing_content() {
        let encoding =
            br#"<?xml version="1.0" encoding="UTF-16"?><svg xmlns="http://www.w3.org/2000/svg"/>"#;
        assert!(matches!(
            read_svg_preview(encoding),
            Err(SvgImportError::UnsupportedXmlDeclaration)
        ));

        let duplicate = br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"><?xml version="1.0"?></svg>"#;
        assert!(read_svg_preview(duplicate).is_err());

        let before = br#"not-xml<svg xmlns="http://www.w3.org/2000/svg"/>"#;
        assert!(matches!(
            read_svg_preview(before),
            Err(SvgImportError::TrailingContent)
        ));
        let after = br#"<svg xmlns="http://www.w3.org/2000/svg"/>not-xml"#;
        assert!(matches!(
            read_svg_preview(after),
            Err(SvgImportError::TrailingContent)
        ));
    }

    #[test]
    fn rejects_trailing_garbage_in_strict_numeric_attributes() {
        let view_box = document(r#"viewBox="0 0 10 10 junk" width="10mm" height="10mm""#, "");
        assert!(matches!(
            read_svg_preview(view_box.as_bytes()),
            Err(SvgImportError::InvalidAttribute { attribute, .. }) if attribute == "viewBox"
        ));

        let aspect = document(
            r#"viewBox="0 0 10 10" width="10mm" height="10mm" preserveAspectRatio="xMidYMid slice junk""#,
            "",
        );
        assert!(matches!(
            read_svg_preview(aspect.as_bytes()),
            Err(SvgImportError::InvalidAttribute { attribute, .. })
                if attribute == "preserveAspectRatio"
        ));

        let points = standard_document(r#"<polyline points="0,0 1,1,"/>"#);
        assert!(matches!(
            read_svg_preview(points.as_bytes()),
            Err(SvgImportError::InvalidAttribute { attribute, .. }) if attribute == "points"
        ));
    }

    #[test]
    fn reports_unsupported_external_and_hidden_content() {
        let preview = preview(
            r#"
                <image href="https://example.invalid/a.png"/>
                <circle cx="1" cy="1" r="1"/>
                <line display="none" x1="0" y1="0" x2="1" y2="1"/>
                <line x1="0" y1="1" x2="1" y2="0"/>
            "#,
        );
        assert!(has_warning(
            &preview,
            &SvgWarningKind::ExternalReferenceIgnored
        ));
        assert!(has_warning(
            &preview,
            &SvgWarningKind::UnsupportedElement("image".to_owned())
        ));
        assert!(has_warning(
            &preview,
            &SvgWarningKind::UnsupportedElement("circle".to_owned())
        ));
        assert!(has_warning(
            &preview,
            &SvgWarningKind::HiddenGeometryIgnored
        ));
    }

    #[test]
    fn enforces_parser_resource_limits() {
        let source = standard_document(r#"<g><g><line x2="1"/></g></g>"#);
        let limits = SvgImportLimits {
            max_depth: 2,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooDeep { .. })
        ));

        let source = standard_document(r#"<line x1="0" y1="0" x2="1" y2="1"/>"#);
        let limits = SvgImportLimits {
            max_elements: 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManyElements { .. })
        ));

        let limits = SvgImportLimits {
            max_attributes_per_element: 2,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::InvalidXml(_)) | Err(SvgImportError::TooManyAttributes { .. })
        ));

        let limits = SvgImportLimits {
            max_file_bytes: source.len() - 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::FileTooLarge { .. })
        ));
    }

    #[test]
    fn enforces_geometry_css_warning_and_candidate_limits() {
        let source = standard_document(r#"<line x2="1"/><line x1="1" x2="2"/>"#);
        let limits = SvgImportLimits {
            max_source_edges: 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManySourceEdges { .. })
        ));

        let source = standard_document(r#"<path d="M0 0 L1 0 L2 0"/>"#);
        let limits = SvgImportLimits {
            max_path_commands: 2,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManyPathCommands { .. })
        ));

        let source = standard_document(
            r#"<style>.a{stroke:red}.b{stroke:blue}</style><line class="a" x2="1"/>"#,
        );
        let limits = SvgImportLimits {
            max_css_rules: 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManyCssRules { .. })
        ));

        let source =
            standard_document(r#"<line class="a" x2="1"/><line class="b" x1="1" x2="2"/>"#);
        let limits = SvgImportLimits {
            max_style_groups: 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManyStyleGroups { .. })
        ));

        let source = standard_document(r#"<rect x="10" y="10" width="20" height="20"/>"#);
        let limits = SvgImportLimits {
            max_boundary_candidates: 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManyBoundaryCandidates { .. })
        ));

        let source = standard_document(r#"<circle/><ellipse/>"#);
        let limits = SvgImportLimits {
            max_warnings: 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManyWarnings { .. })
        ));
    }

    #[test]
    fn bounds_css_rule_element_evaluation_work_at_the_exact_limit() {
        let source = standard_document(
            r#"<style>.fold { stroke: red; }</style><line class="fold" x2="1"/>"#,
        );
        let exact = SvgImportLimits {
            max_css_rule_element_evaluations: 3,
            ..SvgImportLimits::default()
        };
        let preview =
            read_svg_preview_with_limits(source.as_bytes(), exact).expect("exact CSS work limit");
        assert_eq!(preview.edges().len(), 1);

        let one_too_many = SvgImportLimits {
            max_css_rule_element_evaluations: 2,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), one_too_many),
            Err(SvgImportError::TooManyCssRuleElementEvaluations { maximum: 2 })
        ));
    }

    #[test]
    fn bounds_individual_css_selectors_and_property_values() {
        let exact_value = format!("{}1", "0".repeat(MAX_STYLE_VALUE_CHARS - 1));
        let source = standard_document(&format!(
            r#"<line style="stroke-width:{exact_value}" x2="1"/>"#
        ));
        read_svg_preview(source.as_bytes()).expect("exact supported style value limit");

        let oversized_value = format!("{}1", "0".repeat(MAX_STYLE_VALUE_CHARS));
        let source = standard_document(&format!(
            r#"<line style="stroke-width:{oversized_value}" x2="1"/>"#
        ));
        assert!(matches!(
            read_svg_preview(source.as_bytes()),
            Err(SvgImportError::StyleValueTooLong { maximum: 120, .. })
        ));

        let exact_selector = "a".repeat(MAX_CSS_SELECTOR_CHARS);
        let source = standard_document(&format!(
            r#"<style>{exact_selector} {{ stroke: red; }}</style><line x2="1"/>"#
        ));
        read_svg_preview(source.as_bytes()).expect("exact selector limit");

        let oversized_selector = "a".repeat(MAX_CSS_SELECTOR_CHARS + 1);
        let source = standard_document(&format!(
            r#"<style>{oversized_selector} {{ stroke: red; }}</style><line x2="1"/>"#
        ));
        assert!(matches!(
            read_svg_preview(source.as_bytes()),
            Err(SvgImportError::CssSelectorTooLong { maximum: 120 })
        ));
    }

    #[test]
    fn ignores_bounded_unsupported_style_declarations_without_rejecting_geometry() {
        let oversized_unsupported_value = "x".repeat(MAX_STYLE_VALUE_CHARS + 1);
        let unsupported = (0..40)
            .map(|index| format!("vendor-property-{index}:{oversized_unsupported_value}"))
            .collect::<Vec<_>>()
            .join(";");
        let source = standard_document(&format!(
            r#"<line style="{unsupported};stroke:#ff0000 !important" x1="10" y1="10" x2="90" y2="90"/>"#
        ));

        let preview = read_svg_preview(source.as_bytes())
            .expect("bounded unsupported declarations must be ignored");

        assert_eq!(preview.edges().len(), 1);
        assert_eq!(
            preview.style_groups()[0].stroke,
            RgbaColor::opaque(255, 0, 0)
        );
        for index in 0..40 {
            assert!(has_warning(
                &preview,
                &SvgWarningKind::UnsupportedStyleProperty(format!("vendor-property-{index}"))
            ));
        }
    }

    #[test]
    fn unsupported_styles_remain_bounded_by_the_document_style_text_limit() {
        let oversized_style = format!("vendor-property:{}", "x".repeat(MAX_STYLE_TEXT_BYTES));
        let source = standard_document(&format!(
            r#"<line style="{oversized_style}" x1="10" y1="10" x2="90" y2="90"/>"#
        ));

        assert!(matches!(
            read_svg_preview(source.as_bytes()),
            Err(SvgImportError::InvalidCss)
        ));
    }

    #[test]
    fn important_declarations_follow_css_cascade_instead_of_rejecting_the_document() {
        let source = standard_document(
            r#"
                <style>
                    .fold { stroke: #ff0000 !important; }
                    .fold { stroke: #0000ff; }
                    .same-priority { stroke: #0000ff !important; }
                    .same-priority { stroke: #ffff00 !important; }
                </style>
                <line class="fold" style="stroke:#00ff00" x1="10" y1="10" x2="90" y2="90"/>
                <line class="fold" style="stroke:#00ff00 !important" x1="10" y1="90" x2="90" y2="10"/>
                <line class="same-priority" x1="10" y1="50" x2="90" y2="50"/>
            "#,
        );

        let preview = read_svg_preview(source.as_bytes()).expect("important CSS");

        assert_eq!(preview.edges().len(), 3);
        assert_eq!(preview.style_groups().len(), 3);
        assert_eq!(
            preview.style_groups()[0].stroke,
            RgbaColor::opaque(255, 0, 0),
            "stylesheet !important must beat a normal inline declaration"
        );
        assert_eq!(
            preview.style_groups()[1].stroke,
            RgbaColor::opaque(0, 255, 0),
            "inline !important must beat stylesheet !important"
        );
        assert_eq!(
            preview.style_groups()[2].stroke,
            RgbaColor::opaque(255, 255, 0),
            "a later declaration must win at equal specificity and importance"
        );
    }

    #[test]
    fn enforces_intersection_and_final_geometry_limits() {
        let source = standard_document("");
        let limits = SvgImportLimits {
            max_intersection_candidates: 1,
            ..SvgImportLimits::default()
        };
        assert!(matches!(
            read_svg_preview_with_limits(source.as_bytes(), limits),
            Err(SvgImportError::TooManyBoundaryCandidateIntersections { .. })
        ));

        let source = document(
            "",
            r#"
                <line class="boundary" x1="0" y1="0" x2="10" y2="0"/>
                <line class="boundary" x1="10" y1="0" x2="10" y2="10"/>
                <line class="boundary" x1="10" y1="10" x2="0" y2="10"/>
                <line class="boundary" x1="0" y1="10" x2="0" y2="0"/>
                <line class="crease" x1="0" y1="5" x2="10" y2="5"/>
            "#,
        );
        let limits = SvgImportLimits {
            max_intersection_candidates: 0,
            ..SvgImportLimits::default()
        };
        let limited_preview =
            read_svg_preview_with_limits(source.as_bytes(), limits).expect("bounded preview");
        let mut mappings = limited_preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: if group.classes == ["boundary"] {
                    SvgGroupTarget::Boundary
                } else {
                    SvgGroupTarget::Mountain
                },
            })
            .collect::<Vec<_>>();
        mappings.sort_by_key(|mapping| mapping.group);
        let options = SvgConversionOptions {
            millimetres_per_unit: 1.0,
            group_mappings: mappings,
            boundary_candidate: None,
        };
        assert_eq!(
            limited_preview
                .convert(&options)
                .expect_err("intersection cap"),
            SvgConversionError::TooManyIntersectionCandidates { maximum: 0 }
        );

        let preview = preview(r#"<line x1="10" y1="10" x2="20" y2="20"/>"#);
        let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
        let mut constrained = preview.clone();
        constrained.limits.max_final_edges = 3;
        assert_eq!(
            constrained
                .convert(&SvgConversionOptions {
                    millimetres_per_unit: 1.0,
                    group_mappings: constrained
                        .style_groups()
                        .iter()
                        .map(|group| SvgGroupMapping {
                            group: group.id,
                            target: SvgGroupTarget::Ignore,
                        })
                        .collect(),
                    boundary_candidate: Some(view_box),
                })
                .expect_err("edge cap"),
            SvgConversionError::TooManyFinalEdges { maximum: 3 }
        );
    }
}
