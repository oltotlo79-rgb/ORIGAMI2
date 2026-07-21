use std::{collections::HashSet, error::Error, fmt};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod annotations;
mod beginner_candidates;
mod beginner_design;
mod beginner_generation;
mod beginner_generator;
mod beginner_recognition;
mod constraints;
mod element_metadata;
mod layers;
mod underlays;

pub use annotations::{
    ANNOTATION_SCHEMA_VERSION_V1, AnnotationAnchorV1, AnnotationDocumentV1, AnnotationId,
    AnnotationRecordV1, AnnotationStyleV1, MAX_ANNOTATION_FONT_SIZE_MM_V1,
    MAX_ANNOTATION_TEXT_BYTES_V1, MAX_ANNOTATIONS_V1, MIN_ANNOTATION_FONT_SIZE_MM_V1,
    validate_annotation_document_v1,
};
pub use beginner_candidates::{
    BEGINNER_CANDIDATE_SCHEMA_VERSION_V1, BeginnerBulgeTreatmentV1, BeginnerCandidateInputV1,
    BeginnerCandidateKindV1, BeginnerCandidateScoreV1, BeginnerElasticityModelV1,
    MAX_BEGINNER_CANDIDATES_V1, score_beginner_candidates_v1,
};
pub use beginner_design::{
    BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1, BeginnerDesignPresetV1, BeginnerDesignProfileV1,
    BeginnerGenerationProvenanceV1, validate_beginner_design_profile_v1,
    validate_beginner_generation_provenance_v1,
};
pub use beginner_generation::{
    BEGINNER_GENERATION_CONSTRAINTS_SCHEMA_VERSION_V1, BeginnerBodyOutlineModeV1,
    BeginnerBulgeTargetV1, BeginnerDetailLevelV1, BeginnerFoldTechniqueV1,
    BeginnerGenerationConstraintsV1, BeginnerProtrusionJointV1, BeginnerProtrusionSideV1,
    BeginnerProtrusionSymmetryV1, BeginnerProtrusionTargetV1, BeginnerSkeletonPointV1,
    BeginnerSkeletonSegmentV1, BeginnerTargetAssetReferenceV1, BeginnerTargetCategoryV1,
    BeginnerTargetPartKindV1, BeginnerTargetPartRecordV1, MAX_BEGINNER_ALLOWED_TECHNIQUES_V1,
    MAX_BEGINNER_GENERATION_STEPS_V1, MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM_V1,
    MAX_BEGINNER_SKELETON_SEGMENTS_V1, MAX_BEGINNER_SKELETON_THICKNESS_TENTHS_MM_V1,
    MAX_BEGINNER_TARGET_PART_COUNT_V1, MAX_BEGINNER_TARGET_PART_RECORDS_V1,
    MAX_BEGINNER_TARGET_PARTS_TOTAL_V1, MIN_BEGINNER_GENERATION_STEPS_V1,
    validate_beginner_generation_constraints_v1,
};
pub use beginner_generator::{
    BEGINNER_GENERATOR_SCHEMA_VERSION_V1, BEGINNER_PARAMETER_GRID_SIZE_V1,
    BeginnerBilateralPairBindingV1, BeginnerCompleteAnimalBindingV1,
    BeginnerCompleteInsectBindingV1, BeginnerCompleteWingedAnimalBindingV1,
    BeginnerGeneratedPlanKindV1, BeginnerGeneratedPlanV1, BeginnerGeneratorErrorV1,
    BeginnerHornEarBindingV1, BeginnerHornTailBindingV1, BeginnerHornTailEarBindingV1,
    BeginnerParameterGridHashV1, BeginnerParameterGridPointV1,
    BeginnerSymmetricParameterCandidateV1, BeginnerSymmetricParameterEstimateV1,
    BeginnerTailEarBindingV1, BeginnerWingAntennaBindingV1, MAX_BEGINNER_GENERATED_CANDIDATES_V1,
    MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1, animal_complete_bindings_v1,
    animal_complete_winged_bindings_v1, animal_horn_ear_bindings_v1, animal_horn_tail_bindings_v1,
    animal_horn_tail_ear_bindings_v1, animal_tail_ear_bindings_v1, beginner_parameter_grid_hash_v1,
    beginner_parameter_grid_v1, beginner_target_approximation_score_v1,
    estimate_symmetric_parameters_v1, generate_beginner_plans_v1, insect_complete_bindings_v1,
    insect_three_pair_bindings_v1, insect_wing_antenna_bindings_v1,
    symmetric_parameter_candidates_v1,
};
pub use beginner_recognition::{
    BEGINNER_RECOGNITION_SCHEMA_VERSION_V1, BeginnerOutlineCandidateV1,
    BeginnerOutlineConfidenceReasonV1, BeginnerRecognitionBoundsV1, BeginnerRecognitionErrorV1,
    BeginnerRecognitionFormatV1, BeginnerRecognitionProposalV1, MAX_BEGINNER_OUTLINE_CANDIDATES_V1,
    MAX_BEGINNER_RECOGNITION_COMPONENTS_V1, MAX_BEGINNER_RECOGNITION_DIMENSION_V1,
    MAX_BEGINNER_RECOGNITION_PIXELS_V1, analyze_marker_png_rgba_v1,
    analyze_outline_candidates_rgba_v1, analyze_silhouette_png_rgba_v1,
};
pub use constraints::{
    ConstraintId, DEFAULT_MAX_CONSTRAINT_EDGES, DEFAULT_MAX_CONSTRAINT_RECORDS,
    DEFAULT_MAX_CONSTRAINT_REFERENCES, DEFAULT_MAX_CONSTRAINT_VERTICES,
    GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1, GeometricConstraintDocumentV1,
    GeometricConstraintDocumentValidationErrorV1, GeometricConstraintKindV1,
    GeometricConstraintRecordV1, validate_geometric_constraint_document_v1,
};
pub use element_metadata::{
    EdgeMetadataRecordV1, ElementMetadataDocumentV1, ElementMetadataV1,
    ElementMetadataValidationError, FaceMetadataRecordV1, MAX_ELEMENT_MEMO_CHARS,
    MAX_ELEMENT_METADATA_RECORDS, MAX_ELEMENT_NAME_CHARS, VertexMetadataRecordV1,
    validate_element_metadata_document_v1, validate_element_metadata_v1,
};
pub use layers::{
    DEFAULT_PROJECT_LAYER_ID, DEFAULT_PROJECT_LAYER_NAME, EdgeLayerAssignmentV1,
    LayerContentKindV1, LayerRecordV1, MAX_LAYER_EDGE_ASSIGNMENTS, MAX_LAYER_NAME_CHARS,
    MAX_PROJECT_LAYER_INDEX_EDGES, MAX_PROJECT_LAYER_OPACITY, MAX_PROJECT_LAYERS,
    MIN_PROJECT_LAYER_OPACITY, PROJECT_LAYER_SCHEMA_VERSION_V1, ProjectLayerDocumentV1,
    ProjectLayerDocumentValidationErrorV1, validate_project_layer_document_against_pattern_v1,
    validate_project_layer_document_v1,
};
pub use underlays::{
    MAX_UNDERLAY_SCALE_V1, MAX_UNDERLAYS_V1, MIN_UNDERLAY_SCALE_V1, UNDERLAY_SCHEMA_VERSION_V1,
    UnderlayDocumentV1, UnderlayId, UnderlayRecordV1, UnderlayTransformV1,
    validate_underlay_document_v1,
};

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Returns the UUID in its canonical RFC byte order.
            ///
            /// The returned value is an owned copy, so callers can use it in
            /// deterministic keys without borrowing the ID.
            #[must_use]
            pub const fn canonical_bytes(&self) -> [u8; 16] {
                self.0.into_bytes()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

entity_id!(ProjectId);
entity_id!(VertexId);
entity_id!(EdgeId);
entity_id!(FaceId);
entity_id!(AssetId);
entity_id!(InstructionStepId);
entity_id!(LayerId);

impl FaceId {
    /// Derives a stable face ID from a project namespace and canonical name.
    ///
    /// UUID v5 makes the same namespace/name pair deterministic. Callers are
    /// responsible for constructing a collision-resistant canonical name.
    #[must_use]
    pub fn derive_v5(namespace: ProjectId, name: &[u8]) -> Self {
        Self(Uuid::new_v5(&namespace.0, name))
    }
}

impl VertexId {
    /// Derives a stable vertex ID from a project namespace and canonical name.
    #[must_use]
    pub fn derive_v5(namespace: ProjectId, name: &[u8]) -> Self {
        Self(Uuid::new_v5(&namespace.0, name))
    }
}

impl EdgeId {
    /// Derives a stable edge ID from a project namespace and canonical name.
    #[must_use]
    pub fn derive_v5(namespace: ProjectId, name: &[u8]) -> Self {
        Self(Uuid::new_v5(&namespace.0, name))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Mountain,
    Valley,
    Auxiliary,
    Boundary,
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RgbaColor {
    #[must_use]
    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }
}

impl Default for RgbaColor {
    fn default() -> Self {
        Self::opaque(255, 255, 255)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaperAppearance {
    pub color: RgbaColor,
    pub texture_asset: Option<AssetId>,
}

pub const DEFAULT_PAPER_THICKNESS_MM: f64 = 0.10;
pub const DEFAULT_PAPER_FRONT_COLOR: RgbaColor = RgbaColor::opaque(255, 255, 255);
pub const DEFAULT_PAPER_BACK_COLOR: RgbaColor = RgbaColor::opaque(248, 248, 245);

/// Unit used only when presenting or accepting user-visible lengths.
///
/// Persisted geometry and every interchange boundary remain millimetre based.
/// The paper-edge ratio is a live scale whose value `1` is the current length
/// of one explicitly selected boundary edge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthDisplayUnit {
    #[default]
    #[serde(rename = "mm")]
    Millimeter,
    #[serde(rename = "cm")]
    Centimeter,
    #[serde(rename = "inch")]
    Inch,
    #[serde(rename = "paper_edge_ratio")]
    PaperEdgeRatio { reference_edge: EdgeId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paper {
    pub boundary_vertices: Vec<VertexId>,
    /// Physical thickness in millimetres.
    ///
    /// Persistence deliberately neither clamps nor applies a sign policy to
    /// negative or non-finite values. Their admissibility belongs to the
    /// domain-validation workflow; the JSON codec's number representation is
    /// the interchange boundary rather than a reason to mutate design data.
    pub thickness_mm: f64,
    /// User-facing display preference; physical and persisted geometry remains
    /// millimetre based regardless of this value.
    pub length_display_unit: LengthDisplayUnit,
    pub cutting_allowed: bool,
    pub front: PaperAppearance,
    pub back: PaperAppearance,
}

impl Default for Paper {
    fn default() -> Self {
        Self {
            boundary_vertices: Vec::new(),
            thickness_mm: DEFAULT_PAPER_THICKNESS_MM,
            length_display_unit: LengthDisplayUnit::Millimeter,
            cutting_allowed: false,
            front: PaperAppearance {
                color: DEFAULT_PAPER_FRONT_COLOR,
                texture_asset: None,
            },
            back: PaperAppearance {
                color: DEFAULT_PAPER_BACK_COLOR,
                texture_asset: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: VertexId,
    pub position: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub start: VertexId,
    pub end: VertexId,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreasePattern {
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
}

impl CreasePattern {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
        }
    }
}

pub const MAX_INSTRUCTION_STEPS: usize = 512;
pub const MAX_INSTRUCTION_HINGES_PER_STEP: usize = 10_000;
pub const MAX_INSTRUCTION_HINGE_RECORDS: usize = 100_000;
pub const MAX_INSTRUCTION_TITLE_CHARS: usize = 120;
pub const MAX_INSTRUCTION_DESCRIPTION_CHARS: usize = 4_000;
pub const MAX_INSTRUCTION_CAUTION_CHARS: usize = 2_000;
pub const MAX_INSTRUCTION_VISUAL_MARKERS: usize = 64;
pub const MAX_INSTRUCTION_MARKER_LABEL_CHARS: usize = 120;
pub const MIN_INSTRUCTION_DURATION_MS: u32 = 100;
pub const MAX_INSTRUCTION_DURATION_MS: u32 = 600_000;
pub const MIN_INSTRUCTION_ANGLE_DEGREES: f64 = 0.0;
pub const MAX_INSTRUCTION_ANGLE_DEGREES: f64 = 180.0;
pub const FOLD_MODEL_FINGERPRINT_HEX_LENGTH: usize = 64;

/// An ordered collection of authored folding-instruction poses.
///
/// Each pose stores a complete hinge vector rather than a delta from the
/// previous step. This makes individual steps deterministic and independently
/// editable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstructionTimeline {
    pub steps: Vec<InstructionStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstructionStep {
    pub id: InstructionStepId,
    pub title: String,
    pub description: String,
    pub caution: String,
    pub duration_ms: u32,
    #[serde(default)]
    pub visual: InstructionVisual,
    pub pose: InstructionPose,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstructionVisual {
    pub camera: Option<InstructionCamera>,
    pub arrows: Vec<InstructionArrow>,
    pub focus_points: Vec<InstructionFocusPoint>,
    pub hand_guides: Vec<InstructionHandGuide>,
    pub cycle_layer_order_proof_v1: Option<CycleLayerOrderProofV1>,
}

pub const CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1: &str =
    "native_continuous_layer_transport_certificate_v1";
pub const MAX_CYCLE_LAYER_ORDER_PAIRS_V1: usize = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleLayerOrderPairV1 {
    pub lower_face: FaceId,
    pub upper_face: FaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CycleLayerOrderProofV1 {
    pub version: u32,
    pub model_id: String,
    pub target_order_sha256: [u8; 32],
    pub transition_count: usize,
    pub pairs: Vec<CycleLayerOrderPairV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InstructionPoint3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InstructionCamera {
    pub position: InstructionPoint3,
    pub target: InstructionPoint3,
    pub up: InstructionPoint3,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstructionArrow {
    pub start: InstructionPoint3,
    pub end: InstructionPoint3,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstructionFocusPoint {
    pub position: InstructionPoint3,
    pub radius: f64,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionHandGuideKind {
    Pinch,
    Hold,
    Push,
    Regrip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstructionHandGuide {
    pub kind: InstructionHandGuideKind,
    pub position: InstructionPoint3,
    pub direction: InstructionPoint3,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstructionPose {
    pub model: InstructionPoseModel,
    pub source_model_fingerprint: String,
    /// The face held fixed while applying the pose.
    ///
    /// Planar poses do not need a fixed face, so the field is optional.
    pub fixed_face: Option<FaceId>,
    /// Complete hinge angles, strictly ordered by the edge ID's canonical RFC
    /// bytes. Callers must canonicalize before validation or persistence.
    pub hinge_angles: Vec<InstructionHingeAngle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstructionPoseModel {
    AbsoluteHingeAnglesV1,
    /// A human-readable instruction step that deliberately carries no
    /// executable 3D pose. This is used by named-technique templates until a
    /// future, explicitly supported physical operation is authored.
    DeclarativeOnlyV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InstructionHingeAngle {
    pub edge: EdgeId,
    pub angle_degrees: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstructionTimelineValidationError {
    TooManySteps {
        actual: usize,
        maximum: usize,
    },
    TooManyHingeRecords {
        actual: usize,
        maximum: usize,
    },
    DuplicateStepId {
        step_index: usize,
        id: InstructionStepId,
    },
    EmptyTitle {
        step_index: usize,
    },
    TitleTooLong {
        step_index: usize,
        actual: usize,
        maximum: usize,
    },
    TitleContainsControlCharacter {
        step_index: usize,
    },
    DescriptionTooLong {
        step_index: usize,
        actual: usize,
        maximum: usize,
    },
    DescriptionContainsUnsupportedControlCharacter {
        step_index: usize,
    },
    CautionTooLong {
        step_index: usize,
        actual: usize,
        maximum: usize,
    },
    CautionContainsUnsupportedControlCharacter {
        step_index: usize,
    },
    InvalidVisual {
        step_index: usize,
    },
    TooManyVisualMarkers {
        step_index: usize,
        actual: usize,
        maximum: usize,
    },
    DurationOutOfRange {
        step_index: usize,
        actual: u32,
        minimum: u32,
        maximum: u32,
    },
    InvalidSourceModelFingerprint {
        step_index: usize,
    },
    DeclarativePoseHasFixedFace {
        step_index: usize,
    },
    DeclarativePoseHasHingeAngles {
        step_index: usize,
        actual: usize,
    },
    TooManyHingesInStep {
        step_index: usize,
        actual: usize,
        maximum: usize,
    },
    DuplicateHingeEdge {
        step_index: usize,
        edge: EdgeId,
    },
    HingeAnglesNotCanonical {
        step_index: usize,
        previous_edge: EdgeId,
        edge: EdgeId,
    },
    NonFiniteHingeAngle {
        step_index: usize,
        hinge_index: usize,
    },
    HingeAngleOutOfRange {
        step_index: usize,
        hinge_index: usize,
        actual: f64,
        minimum: f64,
        maximum: f64,
    },
}

impl fmt::Display for InstructionTimelineValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManySteps { actual, maximum } => write!(
                formatter,
                "instruction timeline has {actual} steps; the limit is {maximum}"
            ),
            Self::TooManyHingeRecords { actual, maximum } => write!(
                formatter,
                "instruction timeline has {actual} hinge records; the limit is {maximum}"
            ),
            Self::DuplicateStepId { step_index, id } => write!(
                formatter,
                "instruction step {step_index} duplicates step ID {id:?}"
            ),
            Self::EmptyTitle { step_index } => {
                write!(
                    formatter,
                    "instruction step {step_index} has an empty title"
                )
            }
            Self::TitleTooLong {
                step_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "instruction step {step_index} title has {actual} characters; the limit is {maximum}"
            ),
            Self::TitleContainsControlCharacter { step_index } => write!(
                formatter,
                "instruction step {step_index} title contains a control character"
            ),
            Self::DescriptionTooLong {
                step_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "instruction step {step_index} description has {actual} characters; the limit is {maximum}"
            ),
            Self::DescriptionContainsUnsupportedControlCharacter { step_index } => write!(
                formatter,
                "instruction step {step_index} description contains an unsupported control character"
            ),
            Self::CautionTooLong {
                step_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "instruction step {step_index} caution has {actual} characters; the limit is {maximum}"
            ),
            Self::CautionContainsUnsupportedControlCharacter { step_index } => write!(
                formatter,
                "instruction step {step_index} caution contains an unsupported control character"
            ),
            Self::InvalidVisual { step_index } => write!(
                formatter,
                "instruction step {step_index} has an invalid camera or visual marker"
            ),
            Self::TooManyVisualMarkers {
                step_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "instruction step {step_index} has {actual} visual markers; the limit is {maximum}"
            ),
            Self::DurationOutOfRange {
                step_index,
                actual,
                minimum,
                maximum,
            } => write!(
                formatter,
                "instruction step {step_index} duration is {actual} ms; expected {minimum}..={maximum}"
            ),
            Self::InvalidSourceModelFingerprint { step_index } => write!(
                formatter,
                "instruction step {step_index} has an invalid source-model fingerprint"
            ),
            Self::DeclarativePoseHasFixedFace { step_index } => write!(
                formatter,
                "declarative instruction step {step_index} must not specify a fixed face"
            ),
            Self::DeclarativePoseHasHingeAngles { step_index, actual } => write!(
                formatter,
                "declarative instruction step {step_index} has {actual} hinge angles; expected none"
            ),
            Self::TooManyHingesInStep {
                step_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "instruction step {step_index} has {actual} hinges; the limit is {maximum}"
            ),
            Self::DuplicateHingeEdge { step_index, edge } => write!(
                formatter,
                "instruction step {step_index} contains duplicate hinge edge {edge:?}"
            ),
            Self::HingeAnglesNotCanonical {
                step_index,
                previous_edge,
                edge,
            } => write!(
                formatter,
                "instruction step {step_index} hinge edges are not in canonical order: {previous_edge:?} before {edge:?}"
            ),
            Self::NonFiniteHingeAngle {
                step_index,
                hinge_index,
            } => write!(
                formatter,
                "instruction step {step_index} hinge {hinge_index} has a non-finite angle"
            ),
            Self::HingeAngleOutOfRange {
                step_index,
                hinge_index,
                actual,
                minimum,
                maximum,
            } => write!(
                formatter,
                "instruction step {step_index} hinge {hinge_index} angle is {actual}; expected {minimum}..={maximum}"
            ),
        }
    }
}

impl Error for InstructionTimelineValidationError {}

/// Validates resource bounds and model-independent timeline invariants.
///
/// Topology-dependent checks such as whether a face exists, whether every
/// foldable edge is present, and whether the fold model supports the pose are
/// intentionally left to the application/core layer.
pub fn validate_instruction_timeline(
    timeline: &InstructionTimeline,
) -> Result<(), InstructionTimelineValidationError> {
    if timeline.steps.len() > MAX_INSTRUCTION_STEPS {
        return Err(InstructionTimelineValidationError::TooManySteps {
            actual: timeline.steps.len(),
            maximum: MAX_INSTRUCTION_STEPS,
        });
    }

    let total_hinge_records = timeline.steps.iter().fold(0_usize, |total, step| {
        total.saturating_add(step.pose.hinge_angles.len())
    });
    if total_hinge_records > MAX_INSTRUCTION_HINGE_RECORDS {
        return Err(InstructionTimelineValidationError::TooManyHingeRecords {
            actual: total_hinge_records,
            maximum: MAX_INSTRUCTION_HINGE_RECORDS,
        });
    }

    let mut step_ids = HashSet::with_capacity(timeline.steps.len());
    for (step_index, step) in timeline.steps.iter().enumerate() {
        if !step_ids.insert(step.id) {
            return Err(InstructionTimelineValidationError::DuplicateStepId {
                step_index,
                id: step.id,
            });
        }
        validate_instruction_step(step, step_index)?;
    }

    Ok(())
}

fn validate_instruction_step(
    step: &InstructionStep,
    step_index: usize,
) -> Result<(), InstructionTimelineValidationError> {
    if step.title.trim().is_empty() {
        return Err(InstructionTimelineValidationError::EmptyTitle { step_index });
    }
    validate_text(
        &step.title,
        MAX_INSTRUCTION_TITLE_CHARS,
        false,
        || InstructionTimelineValidationError::TitleTooLong {
            step_index,
            actual: step.title.chars().count(),
            maximum: MAX_INSTRUCTION_TITLE_CHARS,
        },
        || InstructionTimelineValidationError::TitleContainsControlCharacter { step_index },
    )?;
    validate_text(
        &step.description,
        MAX_INSTRUCTION_DESCRIPTION_CHARS,
        true,
        || InstructionTimelineValidationError::DescriptionTooLong {
            step_index,
            actual: step.description.chars().count(),
            maximum: MAX_INSTRUCTION_DESCRIPTION_CHARS,
        },
        || InstructionTimelineValidationError::DescriptionContainsUnsupportedControlCharacter {
            step_index,
        },
    )?;
    validate_text(
        &step.caution,
        MAX_INSTRUCTION_CAUTION_CHARS,
        true,
        || InstructionTimelineValidationError::CautionTooLong {
            step_index,
            actual: step.caution.chars().count(),
            maximum: MAX_INSTRUCTION_CAUTION_CHARS,
        },
        || InstructionTimelineValidationError::CautionContainsUnsupportedControlCharacter {
            step_index,
        },
    )?;
    validate_instruction_visual(&step.visual, step_index)?;

    if !(MIN_INSTRUCTION_DURATION_MS..=MAX_INSTRUCTION_DURATION_MS).contains(&step.duration_ms) {
        return Err(InstructionTimelineValidationError::DurationOutOfRange {
            step_index,
            actual: step.duration_ms,
            minimum: MIN_INSTRUCTION_DURATION_MS,
            maximum: MAX_INSTRUCTION_DURATION_MS,
        });
    }

    if !is_lowercase_sha256_hex(&step.pose.source_model_fingerprint) {
        return Err(
            InstructionTimelineValidationError::InvalidSourceModelFingerprint { step_index },
        );
    }

    if step.pose.model == InstructionPoseModel::DeclarativeOnlyV1 {
        if step.pose.fixed_face.is_some() {
            return Err(
                InstructionTimelineValidationError::DeclarativePoseHasFixedFace { step_index },
            );
        }
        if !step.pose.hinge_angles.is_empty() {
            return Err(
                InstructionTimelineValidationError::DeclarativePoseHasHingeAngles {
                    step_index,
                    actual: step.pose.hinge_angles.len(),
                },
            );
        }
    }

    if step.pose.hinge_angles.len() > MAX_INSTRUCTION_HINGES_PER_STEP {
        return Err(InstructionTimelineValidationError::TooManyHingesInStep {
            step_index,
            actual: step.pose.hinge_angles.len(),
            maximum: MAX_INSTRUCTION_HINGES_PER_STEP,
        });
    }

    let mut edge_ids = HashSet::with_capacity(step.pose.hinge_angles.len());
    for (hinge_index, hinge) in step.pose.hinge_angles.iter().enumerate() {
        if !edge_ids.insert(hinge.edge) {
            return Err(InstructionTimelineValidationError::DuplicateHingeEdge {
                step_index,
                edge: hinge.edge,
            });
        }
        if !hinge.angle_degrees.is_finite() {
            return Err(InstructionTimelineValidationError::NonFiniteHingeAngle {
                step_index,
                hinge_index,
            });
        }
        if !(MIN_INSTRUCTION_ANGLE_DEGREES..=MAX_INSTRUCTION_ANGLE_DEGREES)
            .contains(&hinge.angle_degrees)
        {
            return Err(InstructionTimelineValidationError::HingeAngleOutOfRange {
                step_index,
                hinge_index,
                actual: hinge.angle_degrees,
                minimum: MIN_INSTRUCTION_ANGLE_DEGREES,
                maximum: MAX_INSTRUCTION_ANGLE_DEGREES,
            });
        }
    }

    for pair in step.pose.hinge_angles.windows(2) {
        if pair[0].edge.canonical_bytes() >= pair[1].edge.canonical_bytes() {
            return Err(
                InstructionTimelineValidationError::HingeAnglesNotCanonical {
                    step_index,
                    previous_edge: pair[0].edge,
                    edge: pair[1].edge,
                },
            );
        }
    }

    Ok(())
}

fn validate_instruction_visual(
    visual: &InstructionVisual,
    step_index: usize,
) -> Result<(), InstructionTimelineValidationError> {
    if visual
        .cycle_layer_order_proof_v1
        .as_ref()
        .is_some_and(|proof| {
            proof.version != 1
                || proof.model_id != CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1
                || proof.transition_count == 0
                || proof.pairs.len() > MAX_CYCLE_LAYER_ORDER_PAIRS_V1
                || proof
                    .pairs
                    .iter()
                    .any(|pair| pair.lower_face == pair.upper_face)
                || proof.pairs.windows(2).any(|pairs| {
                    (
                        pairs[0].lower_face.canonical_bytes(),
                        pairs[0].upper_face.canonical_bytes(),
                    ) >= (
                        pairs[1].lower_face.canonical_bytes(),
                        pairs[1].upper_face.canonical_bytes(),
                    )
                })
        })
    {
        return Err(InstructionTimelineValidationError::InvalidVisual { step_index });
    }
    let marker_count = visual
        .arrows
        .len()
        .saturating_add(visual.focus_points.len())
        .saturating_add(visual.hand_guides.len());
    if marker_count > MAX_INSTRUCTION_VISUAL_MARKERS {
        return Err(InstructionTimelineValidationError::TooManyVisualMarkers {
            step_index,
            actual: marker_count,
            maximum: MAX_INSTRUCTION_VISUAL_MARKERS,
        });
    }
    let finite = |point: InstructionPoint3| {
        point.x.is_finite() && point.y.is_finite() && point.z.is_finite()
    };
    if let Some(camera) = visual.camera
        && (!finite(camera.position)
            || !finite(camera.target)
            || !finite(camera.up)
            || camera.position == camera.target
            || camera.up
                == (InstructionPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                }))
    {
        return Err(InstructionTimelineValidationError::InvalidVisual { step_index });
    }
    for arrow in &visual.arrows {
        if !finite(arrow.start)
            || !finite(arrow.end)
            || arrow.start == arrow.end
            || arrow.label.chars().count() > MAX_INSTRUCTION_MARKER_LABEL_CHARS
            || arrow.label.chars().any(char::is_control)
        {
            return Err(InstructionTimelineValidationError::InvalidVisual { step_index });
        }
    }
    for focus in &visual.focus_points {
        if !finite(focus.position)
            || !focus.radius.is_finite()
            || focus.radius <= 0.0
            || focus.label.chars().count() > MAX_INSTRUCTION_MARKER_LABEL_CHARS
            || focus.label.chars().any(char::is_control)
        {
            return Err(InstructionTimelineValidationError::InvalidVisual { step_index });
        }
    }
    for guide in &visual.hand_guides {
        if !finite(guide.position)
            || !finite(guide.direction)
            || guide.direction
                == (InstructionPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                })
            || guide.label.chars().count() > MAX_INSTRUCTION_MARKER_LABEL_CHARS
            || guide.label.chars().any(char::is_control)
        {
            return Err(InstructionTimelineValidationError::InvalidVisual { step_index });
        }
    }
    Ok(())
}

fn validate_text(
    text: &str,
    maximum_chars: usize,
    allow_multiline_controls: bool,
    too_long: impl FnOnce() -> InstructionTimelineValidationError,
    invalid_control: impl FnOnce() -> InstructionTimelineValidationError,
) -> Result<(), InstructionTimelineValidationError> {
    if text.chars().count() > maximum_chars {
        return Err(too_long());
    }
    if text.chars().any(|character| {
        character.is_control() && !(allow_multiline_controls && matches!(character, '\n' | '\t'))
    }) {
        return Err(invalid_control());
    }
    Ok(())
}

fn is_lowercase_sha256_hex(value: &str) -> bool {
    value.len() == FOLD_MODEL_FINGERPRINT_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_survive_json_round_trip() {
        let vertex = Vertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 2.0),
        };
        let json = serde_json::to_string(&vertex).expect("serialize vertex");
        let restored: Vertex = serde_json::from_str(&json).expect("deserialize vertex");
        assert_eq!(restored, vertex);
    }

    #[test]
    fn all_entity_ids_expose_canonical_rfc_byte_order() {
        const EXPECTED: [u8; 16] = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        const JSON_ID: &str = r#""00112233-4455-6677-8899-aabbccddeeff""#;

        let project: ProjectId = serde_json::from_str(JSON_ID).expect("deserialize project ID");
        let vertex: VertexId = serde_json::from_str(JSON_ID).expect("deserialize vertex ID");
        let edge: EdgeId = serde_json::from_str(JSON_ID).expect("deserialize edge ID");
        let face: FaceId = serde_json::from_str(JSON_ID).expect("deserialize face ID");
        let asset: AssetId = serde_json::from_str(JSON_ID).expect("deserialize asset ID");
        let instruction_step: InstructionStepId =
            serde_json::from_str(JSON_ID).expect("deserialize instruction step ID");

        for bytes in [
            project.canonical_bytes(),
            vertex.canonical_bytes(),
            edge.canonical_bytes(),
            face.canonical_bytes(),
            asset.canonical_bytes(),
            instruction_step.canonical_bytes(),
        ] {
            assert_eq!(bytes, EXPECTED);
        }
    }

    #[test]
    fn face_v5_derivation_is_deterministic() {
        let namespace: ProjectId =
            serde_json::from_str(r#""00112233-4455-6677-8899-aabbccddeeff""#)
                .expect("deserialize namespace");
        let expected: FaceId = serde_json::from_str(r#""2c99010b-dc57-5a6b-9e5d-9c16280876d7""#)
            .expect("deserialize expected face ID");

        let first = FaceId::derive_v5(namespace, b"face-key");
        let second = FaceId::derive_v5(namespace, b"face-key");

        assert_eq!(first, expected);
        assert_eq!(second, expected);
    }

    #[test]
    fn face_v5_derivation_separates_namespaces_and_names() {
        let first_namespace: ProjectId =
            serde_json::from_str(r#""00112233-4455-6677-8899-aabbccddeeff""#)
                .expect("deserialize first namespace");
        let second_namespace: ProjectId =
            serde_json::from_str(r#""ffffffff-ffff-ffff-ffff-ffffffffffff""#)
                .expect("deserialize second namespace");

        let baseline = FaceId::derive_v5(first_namespace, b"face-key");
        let different_name = FaceId::derive_v5(first_namespace, b"face-key-2");
        let different_namespace = FaceId::derive_v5(second_namespace, b"face-key");

        assert_ne!(baseline, different_name);
        assert_ne!(baseline, different_namespace);
        assert_ne!(different_name, different_namespace);
    }

    #[test]
    fn derived_face_id_survives_json_round_trip() {
        let namespace: ProjectId =
            serde_json::from_str(r#""00112233-4455-6677-8899-aabbccddeeff""#)
                .expect("deserialize namespace");
        let face = FaceId::derive_v5(namespace, b"\0binary\xffface-key");

        let json = serde_json::to_string(&face).expect("serialize derived face ID");
        let restored: FaceId = serde_json::from_str(&json).expect("deserialize derived face ID");

        assert_eq!(restored, face);
        assert_eq!(restored.canonical_bytes(), face.canonical_bytes());
    }

    fn valid_instruction_step() -> InstructionStep {
        let mut hinge_angles = vec![
            InstructionHingeAngle {
                edge: EdgeId::new(),
                angle_degrees: 45.0,
            },
            InstructionHingeAngle {
                edge: EdgeId::new(),
                angle_degrees: 90.0,
            },
        ];
        hinge_angles.sort_by_key(|hinge| hinge.edge.canonical_bytes());
        InstructionStep {
            id: InstructionStepId::new(),
            title: "手順 1".to_owned(),
            description: "谷折りします。\n折り線を合わせます。".to_owned(),
            caution: "ずれに注意\tしてください。".to_owned(),
            duration_ms: 1_500,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: "0123456789abcdef".repeat(4),
                fixed_face: Some(FaceId::new()),
                hinge_angles,
            },
        }
    }

    fn valid_declarative_instruction_step() -> InstructionStep {
        InstructionStep {
            id: InstructionStepId::new(),
            title: "中割り折り（説明）".to_owned(),
            description: "説明テンプレートです。自動実行しません。".to_owned(),
            caution: "層を確認してください。".to_owned(),
            duration_ms: 1_500,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint: "0123456789abcdef".repeat(4),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        }
    }

    #[test]
    fn instruction_timeline_survives_json_round_trip() {
        let mut step = valid_instruction_step();
        step.visual = InstructionVisual {
            camera: Some(InstructionCamera {
                position: InstructionPoint3 {
                    x: 4.0,
                    y: 3.0,
                    z: 5.0,
                },
                target: InstructionPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                up: InstructionPoint3 {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
            }),
            arrows: vec![InstructionArrow {
                start: InstructionPoint3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                end: InstructionPoint3 {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                label: "fold".to_owned(),
            }],
            focus_points: vec![InstructionFocusPoint {
                position: InstructionPoint3 {
                    x: 0.5,
                    y: 0.0,
                    z: 0.0,
                },
                radius: 0.1,
                label: "corner".to_owned(),
            }],
            hand_guides: vec![InstructionHandGuide {
                kind: InstructionHandGuideKind::Pinch,
                position: InstructionPoint3 {
                    x: 0.5,
                    y: 0.0,
                    z: 0.0,
                },
                direction: InstructionPoint3 {
                    x: 0.0,
                    y: -1.0,
                    z: 0.0,
                },
                label: "pinch".to_owned(),
            }],
            cycle_layer_order_proof_v1: Some(CycleLayerOrderProofV1 {
                version: 1,
                model_id: CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1.to_owned(),
                target_order_sha256: [0x5a; 32],
                transition_count: 5,
                pairs: Vec::new(),
            }),
        };
        let timeline = InstructionTimeline { steps: vec![step] };

        validate_instruction_timeline(&timeline).expect("valid timeline");
        let json = serde_json::to_string(&timeline).expect("serialize timeline");
        assert!(json.contains(r#""model":"absolute_hinge_angles_v1""#));
        let restored: InstructionTimeline =
            serde_json::from_str(&json).expect("deserialize timeline");

        assert_eq!(restored, timeline);
        validate_instruction_timeline(&restored).expect("restored timeline");
    }

    #[test]
    fn cycle_layer_order_proof_rejects_stale_model_and_malformed_hash() {
        let mut step = valid_instruction_step();
        step.visual.cycle_layer_order_proof_v1 = Some(CycleLayerOrderProofV1 {
            version: 1,
            model_id: "stale_layer_model".to_owned(),
            target_order_sha256: [0; 32],
            transition_count: 1,
            pairs: Vec::new(),
        });
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline { steps: vec![step] }),
            Err(InstructionTimelineValidationError::InvalidVisual { .. })
        ));
        let malformed = r#"{"version":1,"model_id":"native_continuous_layer_transport_certificate_v1","target_order_sha256":[1],"transition_count":1,"pairs":[]}"#;
        assert!(serde_json::from_str::<CycleLayerOrderProofV1>(malformed).is_err());
    }

    #[test]
    fn legacy_instruction_step_defaults_visuals_and_invalid_visuals_fail_closed() {
        let step = valid_instruction_step();
        let mut json = serde_json::to_value(&step).expect("serialize step");
        json.as_object_mut().expect("step object").remove("visual");
        let restored: InstructionStep = serde_json::from_value(json).expect("legacy step");
        assert_eq!(restored.visual, InstructionVisual::default());

        let mut visual_without_hand_guides =
            serde_json::to_value(&step.visual).expect("serialize visual");
        visual_without_hand_guides
            .as_object_mut()
            .expect("visual object")
            .remove("hand_guides");
        let restored_visual: InstructionVisual =
            serde_json::from_value(visual_without_hand_guides).expect("legacy visual");
        assert!(restored_visual.hand_guides.is_empty());

        let mut invalid = step.clone();
        invalid.visual.focus_points.push(InstructionFocusPoint {
            position: InstructionPoint3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            radius: 0.0,
            label: String::new(),
        });
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![invalid]
            }),
            Err(InstructionTimelineValidationError::InvalidVisual { step_index: 0 })
        ));

        let mut invalid_guide = step;
        invalid_guide.visual.hand_guides.push(InstructionHandGuide {
            kind: InstructionHandGuideKind::Hold,
            position: InstructionPoint3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            direction: InstructionPoint3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            label: "hold".to_owned(),
        });
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![invalid_guide]
            }),
            Err(InstructionTimelineValidationError::InvalidVisual { step_index: 0 })
        ));
    }

    #[test]
    fn declarative_instruction_step_round_trips_without_executable_pose_data() {
        let timeline = InstructionTimeline {
            steps: vec![valid_declarative_instruction_step()],
        };

        validate_instruction_timeline(&timeline).expect("valid declarative timeline");
        let json = serde_json::to_string(&timeline).expect("serialize declarative timeline");
        assert!(json.contains(r#""model":"declarative_only_v1""#));
        let restored: InstructionTimeline =
            serde_json::from_str(&json).expect("deserialize declarative timeline");

        assert_eq!(restored, timeline);
        assert!(restored.steps[0].pose.fixed_face.is_none());
        assert!(restored.steps[0].pose.hinge_angles.is_empty());
    }

    #[test]
    fn declarative_instruction_step_rejects_smuggled_pose_data() {
        let mut fixed = valid_declarative_instruction_step();
        fixed.pose.fixed_face = Some(FaceId::new());
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline { steps: vec![fixed] }),
            Err(InstructionTimelineValidationError::DeclarativePoseHasFixedFace { step_index: 0 })
        ));

        let mut hinged = valid_declarative_instruction_step();
        hinged.pose.hinge_angles.push(InstructionHingeAngle {
            edge: EdgeId::new(),
            angle_degrees: 45.0,
        });
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![hinged]
            }),
            Err(
                InstructionTimelineValidationError::DeclarativePoseHasHingeAngles {
                    step_index: 0,
                    actual: 1
                }
            )
        ));
    }

    #[test]
    fn instruction_timeline_defaults_to_empty() {
        let timeline: InstructionTimeline =
            serde_json::from_str("{}").expect("deserialize default timeline");
        assert!(timeline.steps.is_empty());
        validate_instruction_timeline(&timeline).expect("empty timeline is valid");
    }

    #[test]
    fn instruction_text_limits_count_unicode_characters_and_allow_only_documented_controls() {
        let mut step = valid_instruction_step();
        step.title = "折".repeat(MAX_INSTRUCTION_TITLE_CHARS);
        step.description = "\n\t".to_owned();
        step.caution = "\n\t".to_owned();
        validate_instruction_timeline(&InstructionTimeline {
            steps: vec![step.clone()],
        })
        .expect("limits and supported multiline controls are valid");

        step.title = "有効".to_owned();
        step.description = "説".repeat(MAX_INSTRUCTION_DESCRIPTION_CHARS);
        step.caution = "注".repeat(MAX_INSTRUCTION_CAUTION_CHARS);
        validate_instruction_timeline(&InstructionTimeline {
            steps: vec![step.clone()],
        })
        .expect("description and caution character limits are inclusive");

        step.description.push('明');
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![step.clone()]
            }),
            Err(InstructionTimelineValidationError::DescriptionTooLong { .. })
        ));

        step.description.clear();
        step.caution.push('意');
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![step.clone()]
            }),
            Err(InstructionTimelineValidationError::CautionTooLong { .. })
        ));

        step.description = "\n\t".to_owned();
        step.caution = "\n\t".to_owned();
        step.title = "折".repeat(MAX_INSTRUCTION_TITLE_CHARS);
        step.title.push('る');
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![step.clone()]
            }),
            Err(InstructionTimelineValidationError::TitleTooLong { .. })
        ));

        step.title = "制御\u{0007}文字".to_owned();
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![step.clone()]
            }),
            Err(InstructionTimelineValidationError::TitleContainsControlCharacter { .. })
        ));

        step.title = "有効".to_owned();
        step.description = "制御\r文字".to_owned();
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![step.clone()]
            }),
            Err(
                InstructionTimelineValidationError::DescriptionContainsUnsupportedControlCharacter {
                    ..
                }
            )
        ));

        step.description.clear();
        step.caution = "制御\u{007f}文字".to_owned();
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline { steps: vec![step] }),
            Err(
                InstructionTimelineValidationError::CautionContainsUnsupportedControlCharacter { .. }
            )
        ));
    }

    #[test]
    fn instruction_timeline_rejects_duplicate_ids_empty_title_and_duration_bounds() {
        let step = valid_instruction_step();
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![step.clone(), step.clone()]
            }),
            Err(InstructionTimelineValidationError::DuplicateStepId { step_index: 1, .. })
        ));

        let mut invalid = step.clone();
        invalid.title.clear();
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![invalid]
            }),
            Err(InstructionTimelineValidationError::EmptyTitle { .. })
        ));

        let mut invalid = step.clone();
        invalid.title = " \t\n".to_owned();
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![invalid]
            }),
            Err(InstructionTimelineValidationError::EmptyTitle { .. })
        ));

        for invalid_duration in [
            MIN_INSTRUCTION_DURATION_MS - 1,
            MAX_INSTRUCTION_DURATION_MS + 1,
        ] {
            let mut invalid = step.clone();
            invalid.duration_ms = invalid_duration;
            assert!(matches!(
                validate_instruction_timeline(&InstructionTimeline {
                    steps: vec![invalid]
                }),
                Err(InstructionTimelineValidationError::DurationOutOfRange { .. })
            ));
        }

        let mut minimum = step.clone();
        minimum.duration_ms = MIN_INSTRUCTION_DURATION_MS;
        let mut maximum = step;
        maximum.id = InstructionStepId::new();
        maximum.duration_ms = MAX_INSTRUCTION_DURATION_MS;
        validate_instruction_timeline(&InstructionTimeline {
            steps: vec![minimum, maximum],
        })
        .expect("inclusive duration bounds");
    }

    #[test]
    fn instruction_timeline_rejects_invalid_fingerprint_and_hinge_angles() {
        let mut invalid = valid_instruction_step();
        invalid.pose.source_model_fingerprint = "A".repeat(FOLD_MODEL_FINGERPRINT_HEX_LENGTH);
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![invalid]
            }),
            Err(InstructionTimelineValidationError::InvalidSourceModelFingerprint { .. })
        ));

        let mut invalid = valid_instruction_step();
        invalid.pose.hinge_angles[0].angle_degrees = f64::NAN;
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![invalid]
            }),
            Err(InstructionTimelineValidationError::NonFiniteHingeAngle { .. })
        ));

        for invalid_angle in [
            MIN_INSTRUCTION_ANGLE_DEGREES - 0.1,
            MAX_INSTRUCTION_ANGLE_DEGREES + 0.1,
        ] {
            let mut invalid = valid_instruction_step();
            invalid.pose.hinge_angles[0].angle_degrees = invalid_angle;
            assert!(matches!(
                validate_instruction_timeline(&InstructionTimeline {
                    steps: vec![invalid]
                }),
                Err(InstructionTimelineValidationError::HingeAngleOutOfRange { .. })
            ));
        }

        let mut bounds = valid_instruction_step();
        bounds.pose.hinge_angles[0].angle_degrees = MIN_INSTRUCTION_ANGLE_DEGREES;
        bounds.pose.hinge_angles[1].angle_degrees = MAX_INSTRUCTION_ANGLE_DEGREES;
        validate_instruction_timeline(&InstructionTimeline {
            steps: vec![bounds],
        })
        .expect("inclusive angle bounds");
    }

    #[test]
    fn instruction_timeline_rejects_duplicate_and_noncanonical_hinge_edges() {
        let mut duplicate = valid_instruction_step();
        duplicate.pose.hinge_angles[1].edge = duplicate.pose.hinge_angles[0].edge;
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![duplicate]
            }),
            Err(InstructionTimelineValidationError::DuplicateHingeEdge { .. })
        ));

        let mut noncanonical = valid_instruction_step();
        noncanonical.pose.hinge_angles.reverse();
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![noncanonical]
            }),
            Err(InstructionTimelineValidationError::HingeAnglesNotCanonical { .. })
        ));
    }

    #[test]
    fn instruction_timeline_enforces_step_and_per_step_hinge_limits() {
        let step = valid_instruction_step();
        let maximum_steps = (0..MAX_INSTRUCTION_STEPS)
            .map(|_| valid_instruction_step())
            .collect::<Vec<_>>();
        validate_instruction_timeline(&InstructionTimeline {
            steps: maximum_steps,
        })
        .expect("step limit is inclusive");

        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![step.clone(); MAX_INSTRUCTION_STEPS + 1]
            }),
            Err(InstructionTimelineValidationError::TooManySteps { .. })
        ));

        let mut hinges = (0..=MAX_INSTRUCTION_HINGES_PER_STEP)
            .map(|_| InstructionHingeAngle {
                edge: EdgeId::new(),
                angle_degrees: 0.0,
            })
            .collect::<Vec<_>>();
        hinges.sort_by_key(|hinge| hinge.edge.canonical_bytes());
        let mut invalid = step;
        invalid.pose.hinge_angles = hinges;
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline {
                steps: vec![invalid]
            }),
            Err(InstructionTimelineValidationError::TooManyHingesInStep { .. })
        ));
    }

    #[test]
    fn instruction_timeline_enforces_total_hinge_record_limit() {
        let mut hinges = (0..MAX_INSTRUCTION_HINGES_PER_STEP)
            .map(|_| InstructionHingeAngle {
                edge: EdgeId::new(),
                angle_degrees: 0.0,
            })
            .collect::<Vec<_>>();
        hinges.sort_by_key(|hinge| hinge.edge.canonical_bytes());

        let mut steps = (0..(MAX_INSTRUCTION_HINGE_RECORDS / MAX_INSTRUCTION_HINGES_PER_STEP))
            .map(|_| {
                let mut step = valid_instruction_step();
                step.pose.hinge_angles.clone_from(&hinges);
                step
            })
            .collect::<Vec<_>>();
        validate_instruction_timeline(&InstructionTimeline {
            steps: steps.clone(),
        })
        .expect("exact total hinge limit is valid");

        let mut final_step = valid_instruction_step();
        final_step.pose.hinge_angles.truncate(1);
        steps.push(final_step);
        assert!(matches!(
            validate_instruction_timeline(&InstructionTimeline { steps }),
            Err(InstructionTimelineValidationError::TooManyHingeRecords {
                actual,
                maximum: MAX_INSTRUCTION_HINGE_RECORDS
            }) if actual == MAX_INSTRUCTION_HINGE_RECORDS + 1
        ));
    }

    #[test]
    fn default_paper_is_safe_for_legacy_projects() {
        let paper = Paper::default();
        assert!(paper.boundary_vertices.is_empty());
        assert_eq!(paper.thickness_mm, 0.10);
        assert_eq!(paper.length_display_unit, LengthDisplayUnit::Millimeter);
        assert!(!paper.cutting_allowed);
        assert_eq!(paper.front.color, DEFAULT_PAPER_FRONT_COLOR);
        assert_eq!(paper.back.color, DEFAULT_PAPER_BACK_COLOR);
        assert_eq!(paper.front.texture_asset, None);
        assert_eq!(paper.back.texture_asset, None);
    }

    #[test]
    fn length_display_units_have_a_stable_json_contract() {
        let reference_edge: EdgeId =
            serde_json::from_str(r#""00112233-4455-6677-8899-aabbccddeeff""#)
                .expect("fixed edge ID");

        for (unit, expected) in [
            (LengthDisplayUnit::Millimeter, r#""mm""#),
            (LengthDisplayUnit::Centimeter, r#""cm""#),
            (LengthDisplayUnit::Inch, r#""inch""#),
            (
                LengthDisplayUnit::PaperEdgeRatio { reference_edge },
                r#"{"paper_edge_ratio":{"reference_edge":"00112233-4455-6677-8899-aabbccddeeff"}}"#,
            ),
        ] {
            let json = serde_json::to_string(&unit).expect("serialize display unit");
            assert_eq!(json, expected);
            let restored: LengthDisplayUnit =
                serde_json::from_str(&json).expect("deserialize display unit");
            assert_eq!(restored, unit);
        }
    }

    #[test]
    fn legacy_paper_without_display_unit_defaults_to_millimetres() {
        let paper: Paper = serde_json::from_str("{}").expect("deserialize legacy paper");
        assert_eq!(paper.length_display_unit, LengthDisplayUnit::Millimeter);
    }
}
