//! Strict, versioned project-layer metadata for LIN-004.
//!
//! Existing crease-pattern edges deliberately remain unchanged. An absent
//! assignment means the edge belongs to the reserved default layer; only
//! non-default assignments are stored. This keeps legacy documents compact
//! and gives edge splitting a deterministic inheritance rule.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::{CreasePattern, EdgeId, LayerId};

/// Exact persisted schema version accepted by this implementation.
pub const PROJECT_LAYER_SCHEMA_VERSION_V1: u32 = 1;
/// Non-relaxable number of ordered project layers.
pub const MAX_PROJECT_LAYERS: usize = 256;
/// Non-relaxable number of explicit edge-to-layer assignments.
pub const MAX_LAYER_EDGE_ASSIGNMENTS: usize = 100_000;
/// Non-relaxable crease-pattern edge count while explicit assignments exist.
pub const MAX_PROJECT_LAYER_INDEX_EDGES: usize = 100_000;
/// Maximum number of Unicode scalar values in a layer name.
pub const MAX_LAYER_NAME_CHARS: usize = 120;
/// Stable, language-neutral persisted name used before the author renames it.
pub const DEFAULT_PROJECT_LAYER_NAME: &str = "Crease Pattern";
/// Reserved project-local ID used for every implicit edge assignment.
///
/// The value is intentionally fixed across projects. Layer IDs are scoped by
/// their project, and a fixed non-nil ID makes legacy migration deterministic.
pub const DEFAULT_PROJECT_LAYER_ID: LayerId = LayerId(uuid::Uuid::from_u128(
    0x00000000_0000_4000_8000_000000000001,
));

type CanonicalId = [u8; 16];

/// Declares which object family a layer is allowed to contain.
///
/// V1 edits crease-pattern edge assignments. Annotation and underlay variants
/// reserve strict wire vocabulary for their later object documents, avoiding
/// an ambiguous free-form layer type when those LIN-004 editors are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerContentKindV1 {
    CreasePattern,
    Annotation,
    Underlay,
}

/// One entry in user-controlled back-to-front layer order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerRecordV1 {
    pub id: LayerId,
    pub name: String,
    pub content_kind: LayerContentKindV1,
}

impl LayerRecordV1 {
    /// Creates the one reserved layer used by legacy and empty projects.
    #[must_use]
    pub fn default_crease_pattern() -> Self {
        Self {
            id: DEFAULT_PROJECT_LAYER_ID,
            name: DEFAULT_PROJECT_LAYER_NAME.to_owned(),
            content_kind: LayerContentKindV1::CreasePattern,
        }
    }
}

/// An explicit non-default edge assignment.
///
/// Records must be strictly ordered by the edge ID's canonical UUID bytes.
/// This makes the mapping deterministic and rejects duplicate edge keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeLayerAssignmentV1 {
    pub edge: EdgeId,
    pub layer: LayerId,
}

/// Ordered layer metadata persisted in `project.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectLayerDocumentV1 {
    pub schema_version: u32,
    pub layers: Vec<LayerRecordV1>,
    pub edge_assignments: Vec<EdgeLayerAssignmentV1>,
}

impl ProjectLayerDocumentV1 {
    /// Returns whether serializing this document would add no meaning beyond a
    /// legacy project with one implicit default crease-pattern layer.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.schema_version == PROJECT_LAYER_SCHEMA_VERSION_V1
            && self.layers.as_slice() == [LayerRecordV1::default_crease_pattern()]
            && self.edge_assignments.is_empty()
    }

    /// Resolves an edge assignment without allocating.
    ///
    /// Callers must validate the document first. The canonical ordering then
    /// makes repeated canvas lookups logarithmic instead of quadratic in the
    /// number of displayed edges.
    #[must_use]
    pub fn layer_for_edge(&self, edge: EdgeId) -> LayerId {
        self.edge_assignments
            .binary_search_by_key(&edge.canonical_bytes(), |assignment| {
                assignment.edge.canonical_bytes()
            })
            .map_or(DEFAULT_PROJECT_LAYER_ID, |index| {
                self.edge_assignments[index].layer
            })
    }
}

impl Default for ProjectLayerDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_LAYER_SCHEMA_VERSION_V1,
            layers: vec![LayerRecordV1::default_crease_pattern()],
            edge_assignments: Vec::new(),
        }
    }
}

/// Geometry-independent persisted-layer validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectLayerDocumentValidationErrorV1 {
    UnsupportedSchemaVersion {
        actual: u32,
        expected: u32,
    },
    NoLayers,
    TooManyLayers {
        actual: usize,
        maximum: usize,
    },
    TooManyEdgeAssignments {
        actual: usize,
        maximum: usize,
    },
    TooManyPatternEdges {
        actual: usize,
        maximum: usize,
    },
    AllocationFailed {
        index_name: &'static str,
    },
    NilLayerId {
        layer_index: usize,
    },
    DuplicateLayerId {
        layer: LayerId,
    },
    MissingDefaultLayer,
    DefaultLayerWrongContentKind,
    EmptyLayerName {
        layer: LayerId,
    },
    LayerNameTooLong {
        layer: LayerId,
        actual: usize,
        maximum: usize,
    },
    LayerNameContainsControlCharacter {
        layer: LayerId,
    },
    NilAssignmentEdgeId {
        assignment_index: usize,
    },
    NilAssignmentLayerId {
        assignment_index: usize,
    },
    RedundantDefaultAssignment {
        edge: EdgeId,
    },
    DuplicateEdgeAssignment {
        edge: EdgeId,
    },
    EdgeAssignmentsNotCanonical {
        previous_edge: EdgeId,
        edge: EdgeId,
    },
    MissingAssignmentLayer {
        edge: EdgeId,
        layer: LayerId,
    },
    AssignmentLayerWrongContentKind {
        edge: EdgeId,
        layer: LayerId,
    },
    NilPatternEdgeId {
        edge_index: usize,
    },
    DuplicatePatternEdgeId {
        edge: EdgeId,
    },
    MissingAssignedEdge {
        edge: EdgeId,
    },
}

impl fmt::Display for ProjectLayerDocumentValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported project-layer schema version {actual}; expected {expected}"
            ),
            Self::NoLayers => formatter.write_str("a project must contain its default layer"),
            Self::TooManyLayers { actual, maximum } => write!(
                formatter,
                "project layer count {actual} exceeds the hard maximum {maximum}"
            ),
            Self::TooManyEdgeAssignments { actual, maximum } => write!(
                formatter,
                "edge-layer assignment count {actual} exceeds the hard maximum {maximum}"
            ),
            Self::TooManyPatternEdges { actual, maximum } => write!(
                formatter,
                "crease-pattern edge count {actual} exceeds the layer-index maximum {maximum}"
            ),
            Self::AllocationFailed { index_name } => {
                write!(
                    formatter,
                    "memory for project-layer {index_name} index could not be reserved"
                )
            }
            Self::NilLayerId { layer_index } => {
                write!(formatter, "project layer {layer_index} uses the nil UUID")
            }
            Self::DuplicateLayerId { layer } => {
                write!(
                    formatter,
                    "project layer ID {layer:?} occurs more than once"
                )
            }
            Self::MissingDefaultLayer => {
                formatter.write_str("the reserved default crease-pattern layer is missing")
            }
            Self::DefaultLayerWrongContentKind => {
                formatter.write_str("the reserved default layer must be a crease-pattern layer")
            }
            Self::EmptyLayerName { layer } => {
                write!(formatter, "project layer {layer:?} has an empty name")
            }
            Self::LayerNameTooLong {
                layer,
                actual,
                maximum,
            } => write!(
                formatter,
                "project layer {layer:?} name has {actual} characters; the limit is {maximum}"
            ),
            Self::LayerNameContainsControlCharacter { layer } => write!(
                formatter,
                "project layer {layer:?} name contains a control character"
            ),
            Self::NilAssignmentEdgeId { assignment_index } => write!(
                formatter,
                "edge-layer assignment {assignment_index} uses a nil edge ID"
            ),
            Self::NilAssignmentLayerId { assignment_index } => write!(
                formatter,
                "edge-layer assignment {assignment_index} uses a nil layer ID"
            ),
            Self::RedundantDefaultAssignment { edge } => write!(
                formatter,
                "edge {edge:?} explicitly names the implicit default layer"
            ),
            Self::DuplicateEdgeAssignment { edge } => {
                write!(
                    formatter,
                    "edge {edge:?} has more than one layer assignment"
                )
            }
            Self::EdgeAssignmentsNotCanonical {
                previous_edge,
                edge,
            } => write!(
                formatter,
                "edge-layer assignments are not canonical: {previous_edge:?} precedes {edge:?}"
            ),
            Self::MissingAssignmentLayer { edge, layer } => write!(
                formatter,
                "edge {edge:?} references missing project layer {layer:?}"
            ),
            Self::AssignmentLayerWrongContentKind { edge, layer } => write!(
                formatter,
                "edge {edge:?} cannot be assigned to non-crease layer {layer:?}"
            ),
            Self::NilPatternEdgeId { edge_index } => {
                write!(
                    formatter,
                    "crease-pattern edge {edge_index} uses the nil UUID"
                )
            }
            Self::DuplicatePatternEdgeId { edge } => write!(
                formatter,
                "crease-pattern edge ID {edge:?} occurs more than once while layer assignments exist"
            ),
            Self::MissingAssignedEdge { edge } => {
                write!(
                    formatter,
                    "layer assignment references missing edge {edge:?}"
                )
            }
        }
    }
}

impl Error for ProjectLayerDocumentValidationErrorV1 {}

/// Validates schema, resource limits, identities, names, layer types, and the
/// canonical assignment mapping without inspecting crease-pattern geometry.
pub fn validate_project_layer_document_v1(
    document: &ProjectLayerDocumentV1,
) -> Result<(), ProjectLayerDocumentValidationErrorV1> {
    if document.schema_version != PROJECT_LAYER_SCHEMA_VERSION_V1 {
        return Err(
            ProjectLayerDocumentValidationErrorV1::UnsupportedSchemaVersion {
                actual: document.schema_version,
                expected: PROJECT_LAYER_SCHEMA_VERSION_V1,
            },
        );
    }
    if document.layers.is_empty() {
        return Err(ProjectLayerDocumentValidationErrorV1::NoLayers);
    }
    if document.layers.len() > MAX_PROJECT_LAYERS {
        return Err(ProjectLayerDocumentValidationErrorV1::TooManyLayers {
            actual: document.layers.len(),
            maximum: MAX_PROJECT_LAYERS,
        });
    }
    if document.edge_assignments.len() > MAX_LAYER_EDGE_ASSIGNMENTS {
        return Err(
            ProjectLayerDocumentValidationErrorV1::TooManyEdgeAssignments {
                actual: document.edge_assignments.len(),
                maximum: MAX_LAYER_EDGE_ASSIGNMENTS,
            },
        );
    }

    let mut layer_index = Vec::<(CanonicalId, LayerContentKindV1)>::new();
    layer_index
        .try_reserve_exact(document.layers.len())
        .map_err(
            |_| ProjectLayerDocumentValidationErrorV1::AllocationFailed {
                index_name: "layer",
            },
        )?;
    let mut has_default = false;
    for (index, layer) in document.layers.iter().enumerate() {
        let canonical = layer.id.canonical_bytes();
        if canonical == [0; 16] {
            return Err(ProjectLayerDocumentValidationErrorV1::NilLayerId { layer_index: index });
        }
        if layer.id == DEFAULT_PROJECT_LAYER_ID {
            has_default = true;
            if layer.content_kind != LayerContentKindV1::CreasePattern {
                return Err(ProjectLayerDocumentValidationErrorV1::DefaultLayerWrongContentKind);
            }
        }
        if layer.name.trim().is_empty() {
            return Err(ProjectLayerDocumentValidationErrorV1::EmptyLayerName { layer: layer.id });
        }
        let name_chars = layer.name.chars().count();
        if name_chars > MAX_LAYER_NAME_CHARS {
            return Err(ProjectLayerDocumentValidationErrorV1::LayerNameTooLong {
                layer: layer.id,
                actual: name_chars,
                maximum: MAX_LAYER_NAME_CHARS,
            });
        }
        if layer.name.chars().any(char::is_control) {
            return Err(
                ProjectLayerDocumentValidationErrorV1::LayerNameContainsControlCharacter {
                    layer: layer.id,
                },
            );
        }
        layer_index.push((canonical, layer.content_kind));
    }
    layer_index.sort_unstable_by_key(|(id, _)| *id);
    if let Some(duplicate) = layer_index
        .windows(2)
        .find(|pair| pair[0].0 == pair[1].0)
        .map(|pair| pair[0].0)
    {
        let layer = document
            .layers
            .iter()
            .find(|layer| layer.id.canonical_bytes() == duplicate)
            .expect("the duplicate ID came from a layer")
            .id;
        return Err(ProjectLayerDocumentValidationErrorV1::DuplicateLayerId { layer });
    }
    if !has_default {
        return Err(ProjectLayerDocumentValidationErrorV1::MissingDefaultLayer);
    }

    let mut previous_assignment: Option<(CanonicalId, EdgeId)> = None;
    for (index, assignment) in document.edge_assignments.iter().enumerate() {
        let edge_id = assignment.edge.canonical_bytes();
        if edge_id == [0; 16] {
            return Err(ProjectLayerDocumentValidationErrorV1::NilAssignmentEdgeId {
                assignment_index: index,
            });
        }
        if assignment.layer.canonical_bytes() == [0; 16] {
            return Err(
                ProjectLayerDocumentValidationErrorV1::NilAssignmentLayerId {
                    assignment_index: index,
                },
            );
        }
        if assignment.layer == DEFAULT_PROJECT_LAYER_ID {
            return Err(
                ProjectLayerDocumentValidationErrorV1::RedundantDefaultAssignment {
                    edge: assignment.edge,
                },
            );
        }
        if let Some((previous_id, previous_edge)) = previous_assignment {
            if previous_id == edge_id {
                return Err(
                    ProjectLayerDocumentValidationErrorV1::DuplicateEdgeAssignment {
                        edge: assignment.edge,
                    },
                );
            }
            if previous_id > edge_id {
                return Err(
                    ProjectLayerDocumentValidationErrorV1::EdgeAssignmentsNotCanonical {
                        previous_edge,
                        edge: assignment.edge,
                    },
                );
            }
        }
        previous_assignment = Some((edge_id, assignment.edge));

        let layer_id = assignment.layer.canonical_bytes();
        let Ok(layer_position) =
            layer_index.binary_search_by_key(&layer_id, |(candidate, _)| *candidate)
        else {
            return Err(
                ProjectLayerDocumentValidationErrorV1::MissingAssignmentLayer {
                    edge: assignment.edge,
                    layer: assignment.layer,
                },
            );
        };
        if layer_index[layer_position].1 != LayerContentKindV1::CreasePattern {
            return Err(
                ProjectLayerDocumentValidationErrorV1::AssignmentLayerWrongContentKind {
                    edge: assignment.edge,
                    layer: assignment.layer,
                },
            );
        }
    }
    Ok(())
}

/// Validates a layer document and every explicit edge reference against the
/// supplied crease pattern.
///
/// Legacy/default documents return before constructing an edge index, so a
/// repairable old crease pattern is not rejected merely because layers were
/// introduced later.
pub fn validate_project_layer_document_against_pattern_v1(
    document: &ProjectLayerDocumentV1,
    pattern: &CreasePattern,
) -> Result<(), ProjectLayerDocumentValidationErrorV1> {
    validate_project_layer_document_v1(document)?;
    if document.edge_assignments.is_empty() {
        return Ok(());
    }
    if pattern.edges.len() > MAX_PROJECT_LAYER_INDEX_EDGES {
        return Err(ProjectLayerDocumentValidationErrorV1::TooManyPatternEdges {
            actual: pattern.edges.len(),
            maximum: MAX_PROJECT_LAYER_INDEX_EDGES,
        });
    }

    let mut edges = Vec::<CanonicalId>::new();
    edges.try_reserve_exact(pattern.edges.len()).map_err(|_| {
        ProjectLayerDocumentValidationErrorV1::AllocationFailed {
            index_name: "crease-pattern-edge",
        }
    })?;
    for (index, edge) in pattern.edges.iter().enumerate() {
        let id = edge.id.canonical_bytes();
        if id == [0; 16] {
            return Err(ProjectLayerDocumentValidationErrorV1::NilPatternEdgeId {
                edge_index: index,
            });
        }
        edges.push(id);
    }
    edges.sort_unstable();
    if let Some(duplicate) = edges.windows(2).find(|pair| pair[0] == pair[1]) {
        let edge = pattern
            .edges
            .iter()
            .find(|edge| edge.id.canonical_bytes() == duplicate[0])
            .expect("the duplicate ID came from a crease-pattern edge")
            .id;
        return Err(ProjectLayerDocumentValidationErrorV1::DuplicatePatternEdgeId { edge });
    }

    for assignment in &document.edge_assignments {
        if edges
            .binary_search(&assignment.edge.canonical_bytes())
            .is_err()
        {
            return Err(ProjectLayerDocumentValidationErrorV1::MissingAssignedEdge {
                edge: assignment.edge,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{Edge, EdgeKind, VertexId};

    fn nil_layer_id() -> LayerId {
        serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"").expect("nil layer fixture")
    }

    fn nil_edge_id() -> EdgeId {
        serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"").expect("nil edge fixture")
    }

    fn layer(name: &str, content_kind: LayerContentKindV1) -> LayerRecordV1 {
        LayerRecordV1 {
            id: LayerId::new(),
            name: name.to_owned(),
            content_kind,
        }
    }

    fn edge(id: EdgeId) -> Edge {
        Edge {
            id,
            start: VertexId::new(),
            end: VertexId::new(),
            kind: EdgeKind::Mountain,
        }
    }

    #[test]
    fn default_document_has_a_stable_reserved_layer_and_is_semantically_empty() {
        let first = ProjectLayerDocumentV1::default();
        let second = ProjectLayerDocumentV1::default();
        assert_eq!(first, second);
        assert!(first.is_default());
        assert_eq!(first.layers[0].id, DEFAULT_PROJECT_LAYER_ID);
        assert_eq!(
            serde_json::to_value(&first).expect("serialize default layer document"),
            json!({
                "schema_version": 1,
                "layers": [{
                    "id": "00000000-0000-4000-8000-000000000001",
                    "name": "Crease Pattern",
                    "content_kind": "crease_pattern"
                }],
                "edge_assignments": []
            })
        );
        validate_project_layer_document_v1(&first).expect("default document validates");
    }

    #[test]
    fn strict_round_trip_preserves_all_reserved_content_kinds() {
        let mut document = ProjectLayerDocumentV1::default();
        let crease = layer("Details", LayerContentKindV1::CreasePattern);
        let annotation = layer("Notes", LayerContentKindV1::Annotation);
        let underlay = layer("Reference", LayerContentKindV1::Underlay);
        let edge_id = EdgeId::new();
        document
            .layers
            .extend([crease.clone(), annotation.clone(), underlay.clone()]);
        document.edge_assignments.push(EdgeLayerAssignmentV1 {
            edge: edge_id,
            layer: crease.id,
        });

        validate_project_layer_document_v1(&document).expect("valid authored layers");
        let bytes = serde_json::to_vec(&document).expect("serialize");
        let decoded: ProjectLayerDocumentV1 = serde_json::from_slice(&bytes).expect("deserialize");
        assert_eq!(decoded, document);
        assert_eq!(decoded.layer_for_edge(edge_id), crease.id);
        assert_eq!(
            decoded.layer_for_edge(EdgeId::new()),
            DEFAULT_PROJECT_LAYER_ID
        );
    }

    #[test]
    fn serde_rejects_unknown_fields_and_unknown_content_kinds() {
        let mut value =
            serde_json::to_value(ProjectLayerDocumentV1::default()).expect("serialize fixture");
        value["unexpected"] = json!(true);
        assert!(serde_json::from_value::<ProjectLayerDocumentV1>(value).is_err());

        let mut value =
            serde_json::to_value(ProjectLayerDocumentV1::default()).expect("serialize fixture");
        value["layers"][0]["unexpected"] = json!(true);
        assert!(serde_json::from_value::<ProjectLayerDocumentV1>(value).is_err());

        let mut value =
            serde_json::to_value(ProjectLayerDocumentV1::default()).expect("serialize fixture");
        value["layers"][0]["content_kind"] = json!("future_kind");
        assert!(serde_json::from_value::<ProjectLayerDocumentV1>(value).is_err());

        let mut value =
            serde_json::to_value(ProjectLayerDocumentV1::default()).expect("serialize fixture");
        value["edge_assignments"] = json!([{
            "edge": EdgeId::new(),
            "layer": LayerId::new(),
            "unexpected": true
        }]);
        assert!(serde_json::from_value::<ProjectLayerDocumentV1>(value).is_err());
    }

    #[test]
    fn intrinsic_validation_rejects_identity_name_and_reference_failures() {
        let mut document = ProjectLayerDocumentV1::default();
        document.schema_version += 1;
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::UnsupportedSchemaVersion { .. })
        ));

        let mut document = ProjectLayerDocumentV1::default();
        document.layers.clear();
        assert_eq!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::NoLayers)
        );

        let mut document = ProjectLayerDocumentV1::default();
        document.layers[0].id = nil_layer_id();
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::NilLayerId { .. })
        ));

        let mut document = ProjectLayerDocumentV1::default();
        document.layers.push(document.layers[0].clone());
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::DuplicateLayerId { .. })
        ));

        let mut document = ProjectLayerDocumentV1::default();
        document.layers[0].id = LayerId::new();
        assert_eq!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::MissingDefaultLayer)
        );

        let mut document = ProjectLayerDocumentV1::default();
        document.layers[0].content_kind = LayerContentKindV1::Annotation;
        assert_eq!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::DefaultLayerWrongContentKind)
        );

        let mut document = ProjectLayerDocumentV1::default();
        document.layers[0].name = "\u{2003}".to_owned();
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::EmptyLayerName { .. })
        ));

        let mut document = ProjectLayerDocumentV1::default();
        document.layers[0].name = "x".repeat(MAX_LAYER_NAME_CHARS + 1);
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::LayerNameTooLong { .. })
        ));

        let mut document = ProjectLayerDocumentV1::default();
        document.layers[0].name = "line\nbreak".to_owned();
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::LayerNameContainsControlCharacter { .. })
        ));
    }

    #[test]
    fn assignment_validation_rejects_nil_duplicate_noncanonical_and_wrong_kind() {
        let mut document = ProjectLayerDocumentV1::default();
        let crease = layer("Details", LayerContentKindV1::CreasePattern);
        let annotation = layer("Notes", LayerContentKindV1::Annotation);
        document.layers.extend([crease.clone(), annotation.clone()]);

        document.edge_assignments = vec![EdgeLayerAssignmentV1 {
            edge: nil_edge_id(),
            layer: crease.id,
        }];
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::NilAssignmentEdgeId { .. })
        ));

        document.edge_assignments[0].edge = EdgeId::new();
        document.edge_assignments[0].layer = nil_layer_id();
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::NilAssignmentLayerId { .. })
        ));

        document.edge_assignments[0].layer = DEFAULT_PROJECT_LAYER_ID;
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::RedundantDefaultAssignment { .. })
        ));

        document.edge_assignments[0].layer = LayerId::new();
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::MissingAssignmentLayer { .. })
        ));

        document.edge_assignments[0].layer = annotation.id;
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::AssignmentLayerWrongContentKind { .. })
        ));

        let first = EdgeId::new();
        document.edge_assignments = vec![
            EdgeLayerAssignmentV1 {
                edge: first,
                layer: crease.id,
            },
            EdgeLayerAssignmentV1 {
                edge: first,
                layer: crease.id,
            },
        ];
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::DuplicateEdgeAssignment { .. })
        ));

        let mut ids = [EdgeId::new(), EdgeId::new()];
        ids.sort_unstable_by_key(EdgeId::canonical_bytes);
        document.edge_assignments = ids
            .into_iter()
            .rev()
            .map(|edge| EdgeLayerAssignmentV1 {
                edge,
                layer: crease.id,
            })
            .collect();
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::EdgeAssignmentsNotCanonical { .. })
        ));
    }

    #[test]
    fn assignment_pattern_validation_rejects_dangling_and_ambiguous_edges() {
        let mut document = ProjectLayerDocumentV1::default();
        let crease = layer("Details", LayerContentKindV1::CreasePattern);
        document.layers.push(crease.clone());
        let assigned = EdgeId::new();
        document.edge_assignments.push(EdgeLayerAssignmentV1 {
            edge: assigned,
            layer: crease.id,
        });

        let pattern = CreasePattern {
            vertices: Vec::new(),
            edges: Vec::new(),
        };
        assert_eq!(
            validate_project_layer_document_against_pattern_v1(&document, &pattern),
            Err(ProjectLayerDocumentValidationErrorV1::MissingAssignedEdge { edge: assigned })
        );

        let pattern = CreasePattern {
            vertices: Vec::new(),
            edges: vec![edge(assigned), edge(assigned)],
        };
        assert_eq!(
            validate_project_layer_document_against_pattern_v1(&document, &pattern),
            Err(ProjectLayerDocumentValidationErrorV1::DuplicatePatternEdgeId { edge: assigned })
        );

        let pattern = CreasePattern {
            vertices: Vec::new(),
            edges: vec![edge(assigned)],
        };
        validate_project_layer_document_against_pattern_v1(&document, &pattern)
            .expect("present unique assignment target");
    }

    #[test]
    fn assignment_order_property_is_stable_for_many_generated_ids() {
        let mut document = ProjectLayerDocumentV1::default();
        let crease = layer("Details", LayerContentKindV1::CreasePattern);
        document.layers.push(crease.clone());
        let mut edges = (0..512).map(|_| EdgeId::new()).collect::<Vec<_>>();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        document.edge_assignments = edges
            .iter()
            .copied()
            .map(|edge| EdgeLayerAssignmentV1 {
                edge,
                layer: crease.id,
            })
            .collect();
        validate_project_layer_document_v1(&document).expect("canonical generated mapping");

        for (assignment, edge) in document.edge_assignments.iter().zip(edges) {
            assert_eq!(assignment.edge, edge);
            assert_eq!(document.layer_for_edge(edge), crease.id);
        }
    }

    #[test]
    fn hard_collection_limits_are_enforced_before_indexing() {
        let mut document = ProjectLayerDocumentV1::default();
        document.layers.extend(
            (document.layers.len()..=MAX_PROJECT_LAYERS)
                .map(|index| layer(&format!("Layer {index}"), LayerContentKindV1::Annotation)),
        );
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::TooManyLayers { .. })
        ));

        let document = ProjectLayerDocumentV1 {
            edge_assignments: (0..=MAX_LAYER_EDGE_ASSIGNMENTS)
                .map(|_| EdgeLayerAssignmentV1 {
                    edge: EdgeId::new(),
                    layer: LayerId::new(),
                })
                .collect(),
            ..ProjectLayerDocumentV1::default()
        };
        assert!(matches!(
            validate_project_layer_document_v1(&document),
            Err(ProjectLayerDocumentValidationErrorV1::TooManyEdgeAssignments { .. })
        ));
    }
}
