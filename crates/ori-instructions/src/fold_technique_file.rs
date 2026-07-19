//! Version-fixed, declarative files for sharing named folding techniques.
//!
//! A validated file is still inert data. It does not contain an executable
//! expression language, a filesystem path, a fetchable URL, a script hook, or
//! project-mutation authority. In particular, describing an inside reverse,
//! outside reverse, or sink fold does not claim that the V1 simulator can
//! perform its layer-selective physical motion.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Exact schema identifier accepted by the V1 reader.
pub const FOLD_TECHNIQUE_FILE_SCHEMA_V1: &str = "origami2_fold_technique_file";
/// Exact schema version accepted by the V1 reader.
pub const FOLD_TECHNIQUE_FILE_VERSION_V1: u32 = 1;
/// Non-relaxable maximum encoded size of a shared technique file.
pub const MAX_FOLD_TECHNIQUE_FILE_BYTES: usize = 1024 * 1024;

const MAX_JSON_STRUCTURAL_DEPTH: usize = 32;
const MAX_PACKAGE_ID_BYTES: usize = 96;
const MAX_TECHNIQUES: usize = 64;
const MAX_AUTHORS: usize = 8;
const MAX_AUTHOR_NAME_CHARS: usize = 120;
const MAX_AUTHOR_NAME_BYTES: usize = 480;
const MAX_SOURCE_CITATION_CHARS: usize = 1_024;
const MAX_SOURCE_CITATION_BYTES: usize = 4_096;
const MAX_LICENSE_ID_BYTES: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 96;
const MAX_LOCALES: usize = 8;
const MAX_LOCALE_BYTES: usize = 35;
const MAX_NAME_CHARS: usize = 120;
const MAX_NAME_BYTES: usize = 480;
const MAX_DESCRIPTION_CHARS: usize = 2_048;
const MAX_DESCRIPTION_BYTES: usize = 8_192;
const MAX_PARAMETERS: usize = 64;
const MAX_CHOICES: usize = 32;
const MAX_PRECONDITIONS: usize = 128;
const MAX_PRECONDITION_DEPTH: usize = 8;
const MAX_PRECONDITION_NODES_PER_TECHNIQUE: usize = 512;
const MAX_OPERATIONS: usize = 256;
const MAX_OPERATION_PARAMETER_BINDINGS: usize = 32;
const MAX_OPERATION_PRECONDITIONS: usize = 32;
const MAX_OPERATION_CAPABILITIES: usize = 8;
const MAX_TECHNIQUE_VERSION: u32 = 1_000_000;
const MAX_LENGTH_MICROMETRES: i64 = 10_000_000_000;
const MAX_ABSOLUTE_ANGLE_MICRODEGREES: i64 = 180_000_000;
const MAX_RATIO_MILLIONTHS: i64 = 1_000_000_000;
const MAX_ABSOLUTE_INTEGER: i64 = 1_000_000_000;

/// Strict wire document. Constructed documents must pass
/// [`validate_fold_technique_file_v1`] before they can be written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueFileDocumentV1 {
    pub schema: String,
    pub version: u32,
    pub package_id: String,
    pub metadata: FoldTechniqueMetadataV1,
    pub techniques: Vec<FoldTechniqueTemplateV1>,
}

/// Inert attribution metadata. `source` text is never dereferenced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueMetadataV1 {
    pub authors: Vec<String>,
    pub source: FoldTechniqueSourceV1,
    pub license_spdx_id: String,
}

/// Provenance without a path or fetchable-resource field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FoldTechniqueSourceV1 {
    UserAuthored,
    Adapted { citation_text: String },
    PublishedReference { citation_text: String },
}

/// One named and versioned declarative technique.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueTemplateV1 {
    pub id: String,
    pub version: u32,
    pub names: Vec<FoldTechniqueLocalizedTextV1>,
    pub descriptions: Vec<FoldTechniqueLocalizedTextV1>,
    pub parameters: Vec<FoldTechniqueParameterDefinitionV1>,
    pub preconditions: Vec<FoldTechniquePreconditionDefinitionV1>,
    /// Author-defined order is meaningful and is preserved.
    pub operations: Vec<FoldTechniqueOperationV1>,
}

/// A bounded locale/text pair. Locale identifiers use a canonical lowercase
/// BCP-47-compatible subset such as `ja`, `en`, or `ja-jp`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueLocalizedTextV1 {
    pub locale: String,
    pub text: String,
}

/// A typed parameter definition. Numeric values are fixed-point integers, not
/// executable expressions or binary floating-point text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueParameterDefinitionV1 {
    pub id: String,
    pub names: Vec<FoldTechniqueLocalizedTextV1>,
    pub descriptions: Vec<FoldTechniqueLocalizedTextV1>,
    pub parameter_type: FoldTechniqueParameterTypeV1,
}

/// Closed V1 parameter types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FoldTechniqueParameterTypeV1 {
    LengthMicrometres {
        minimum: i64,
        maximum: i64,
        default: i64,
    },
    AngleMicrodegrees {
        minimum: i64,
        maximum: i64,
        default: i64,
    },
    RatioMillionths {
        minimum: i64,
        maximum: i64,
        default: i64,
    },
    Integer {
        minimum: i64,
        maximum: i64,
        default: i64,
    },
    Boolean {
        default: bool,
    },
    Choice {
        options: Vec<FoldTechniqueChoiceOptionV1>,
        default_option_id: String,
    },
}

/// One ordered choice exposed by a typed choice parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueChoiceOptionV1 {
    pub id: String,
    pub names: Vec<FoldTechniqueLocalizedTextV1>,
}

/// Named precondition referenced by operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniquePreconditionDefinitionV1 {
    pub id: String,
    pub condition: FoldTechniquePreconditionV1,
}

/// Closed, bounded condition AST. There is intentionally no source-expression
/// string, function call, interpolation, or variable lookup node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FoldTechniquePreconditionV1 {
    All {
        conditions: Vec<FoldTechniquePreconditionV1>,
    },
    Any {
        conditions: Vec<FoldTechniquePreconditionV1>,
    },
    Not {
        condition: Box<FoldTechniquePreconditionV1>,
    },
    ParameterComparison {
        parameter_id: String,
        comparison: FoldTechniqueComparisonV1,
        value: FoldTechniqueParameterLiteralV1,
    },
    CapabilityAvailable {
        capability: FoldTechniqueCapabilityV1,
    },
    UserConfirmation {
        prompts: Vec<FoldTechniqueLocalizedTextV1>,
    },
}

/// Comparisons allowed by the closed precondition AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldTechniqueComparisonV1 {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// Typed literal used only in a parameter comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FoldTechniqueParameterLiteralV1 {
    LengthMicrometres { value: i64 },
    AngleMicrodegrees { value: i64 },
    RatioMillionths { value: i64 },
    Integer { value: i64 },
    Boolean { value: bool },
    Choice { option_id: String },
}

/// One entry in the ordered operation template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueOperationV1 {
    pub id: String,
    pub names: Vec<FoldTechniqueLocalizedTextV1>,
    pub action: FoldTechniqueActionV1,
    pub parameter_bindings: Vec<FoldTechniqueParameterBindingV1>,
    pub precondition_ids: Vec<String>,
    pub required_capabilities: Vec<FoldTechniqueCapabilityV1>,
    pub execution_support: FoldTechniqueExecutionSupportV1,
}

/// A semantic role bound to one declared parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FoldTechniqueParameterBindingV1 {
    pub role: String,
    pub parameter_id: String,
}

/// Declarative actions that can be named in V1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FoldTechniqueActionV1 {
    InstructionCue {
        instructions: Vec<FoldTechniqueLocalizedTextV1>,
    },
    StraightLineStackedFold,
    InsideReverseFold,
    OutsideReverseFold,
    SinkFold {
        sink_kind: FoldTechniqueSinkKindV1,
    },
    LayerSelectiveManipulation {
        instructions: Vec<FoldTechniqueLocalizedTextV1>,
    },
}

/// Sink-fold metadata; both variants remain descriptive in V1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldTechniqueSinkKindV1 {
    Open,
    Closed,
}

/// Explicit capability vocabulary. Presence in a shared file is a requirement
/// to be checked by a future host; it is never a capability grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldTechniqueCapabilityV1 {
    HumanInterpretationV1,
    InstructionTimelineV1,
    ManualPoseRegistrationV1,
    StraightLineStackedFoldV1,
    LayerSelectiveMotionV1,
    InsideReverseFoldMotionV1,
    OutsideReverseFoldMotionV1,
    SinkFoldMotionV1,
}

/// V1 has no `supported` or `executable` value. The value describes why an
/// operation is inert or which physical motion the current simulator lacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FoldTechniqueExecutionSupportV1 {
    DeclarativeOnly,
    UnsupportedPhysicalOperation {
        operation: FoldTechniqueUnsupportedPhysicalOperationV1,
    },
}

/// Physical operations intentionally outside the initial SIM-010 scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldTechniqueUnsupportedPhysicalOperationV1 {
    LayerSelectiveMotionV1,
    InsideReverseFoldMotionV1,
    OutsideReverseFoldMotionV1,
    SinkFoldMotionV1,
}

/// Validated and canonically ordered inert file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldTechniqueFileV1 {
    document: FoldTechniqueFileDocumentV1,
}

impl FoldTechniqueFileV1 {
    /// Returns the validated canonical document for read-only inspection.
    pub fn document(&self) -> &FoldTechniqueFileDocumentV1 {
        &self.document
    }

    /// Shared technique data never grants project-mutation authority.
    pub const fn grants_project_mutation_authority(&self) -> bool {
        false
    }

    /// Shared technique data never enables code or script execution.
    pub const fn permits_code_execution(&self) -> bool {
        false
    }

    /// Source citations are inert text and are never fetched automatically.
    pub const fn permits_external_resource_access(&self) -> bool {
        false
    }
}

/// Stable error categories. Untrusted strings are not reflected into errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FoldTechniqueFileError {
    #[error("fold technique file exceeds its encoded size limit")]
    InputTooLarge,
    #[error("fold technique JSON nesting is too deep")]
    JsonNestingTooDeep,
    #[error("fold technique file is not strict V1 JSON")]
    InvalidJson,
    #[error("fold technique file uses an unsupported schema")]
    UnsupportedSchema,
    #[error("fold technique file uses an unsupported version")]
    UnsupportedVersion,
    #[error("fold technique file exceeds a fixed resource limit: {resource}")]
    ResourceLimitExceeded { resource: &'static str },
    #[error("fold technique file contains an invalid field: {field}")]
    InvalidField { field: &'static str },
    #[error("fold technique file contains a duplicate identifier: {kind}")]
    DuplicateIdentifier { kind: &'static str },
    #[error("fold technique file contains a missing reference: {kind}")]
    MissingReference { kind: &'static str },
    #[error("fold technique parameter literal has the wrong type or range")]
    ParameterTypeMismatch,
    #[error("fold technique physical support metadata is inconsistent")]
    InconsistentExecutionSupport,
    #[error("fold technique file serialization failed")]
    SerializationFailed,
    #[error("fold technique file failed deterministic read-back validation")]
    ReadBackMismatch,
}

/// Parses, strictly validates, and canonically orders an untrusted V1 file.
pub fn read_fold_technique_file_v1(
    bytes: &[u8],
) -> Result<FoldTechniqueFileV1, FoldTechniqueFileError> {
    ensure_input_boundary(bytes)?;
    let document = serde_json::from_slice::<FoldTechniqueFileDocumentV1>(bytes)
        .map_err(|_| FoldTechniqueFileError::InvalidJson)?;
    validate_fold_technique_file_v1(document)
}

/// Validates and canonically orders a caller-created wire document.
pub fn validate_fold_technique_file_v1(
    mut document: FoldTechniqueFileDocumentV1,
) -> Result<FoldTechniqueFileV1, FoldTechniqueFileError> {
    validate_document(&mut document)?;
    let canonical_bytes =
        serde_json::to_vec(&document).map_err(|_| FoldTechniqueFileError::SerializationFailed)?;
    if canonical_bytes.len() > MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err(FoldTechniqueFileError::InputTooLarge);
    }
    Ok(FoldTechniqueFileV1 { document })
}

/// Produces compact deterministic JSON and independently reads it back before
/// returning any bytes.
pub fn write_fold_technique_file_v1(
    file: &FoldTechniqueFileV1,
) -> Result<Vec<u8>, FoldTechniqueFileError> {
    let bytes = serde_json::to_vec(&file.document)
        .map_err(|_| FoldTechniqueFileError::SerializationFailed)?;
    if bytes.len() > MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err(FoldTechniqueFileError::InputTooLarge);
    }
    let restored = read_fold_technique_file_v1(&bytes)?;
    if &restored != file {
        return Err(FoldTechniqueFileError::ReadBackMismatch);
    }
    Ok(bytes)
}

fn ensure_input_boundary(bytes: &[u8]) -> Result<(), FoldTechniqueFileError> {
    if bytes.len() > MAX_FOLD_TECHNIQUE_FILE_BYTES {
        return Err(FoldTechniqueFileError::InputTooLarge);
    }
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth
                    .checked_add(1)
                    .ok_or(FoldTechniqueFileError::JsonNestingTooDeep)?;
                if depth > MAX_JSON_STRUCTURAL_DEPTH {
                    return Err(FoldTechniqueFileError::JsonNestingTooDeep);
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn validate_document(
    document: &mut FoldTechniqueFileDocumentV1,
) -> Result<(), FoldTechniqueFileError> {
    if document.schema != FOLD_TECHNIQUE_FILE_SCHEMA_V1 {
        return Err(FoldTechniqueFileError::UnsupportedSchema);
    }
    if document.version != FOLD_TECHNIQUE_FILE_VERSION_V1 {
        return Err(FoldTechniqueFileError::UnsupportedVersion);
    }
    validate_identifier(&document.package_id, MAX_PACKAGE_ID_BYTES, "package_id")?;
    validate_metadata(&mut document.metadata)?;
    ensure_nonempty_limit(document.techniques.len(), MAX_TECHNIQUES, "techniques")?;
    let mut ids = HashSet::with_capacity(document.techniques.len());
    for technique in &mut document.techniques {
        validate_identifier(&technique.id, MAX_IDENTIFIER_BYTES, "technique.id")?;
        if !ids.insert(technique.id.clone()) {
            return Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "technique.id",
            });
        }
        validate_technique(technique)?;
    }
    document
        .techniques
        .sort_by(|left, right| left.id.cmp(&right.id));
    Ok(())
}

fn validate_metadata(metadata: &mut FoldTechniqueMetadataV1) -> Result<(), FoldTechniqueFileError> {
    ensure_nonempty_limit(metadata.authors.len(), MAX_AUTHORS, "metadata.authors")?;
    for author in &metadata.authors {
        validate_text(
            author,
            MAX_AUTHOR_NAME_CHARS,
            MAX_AUTHOR_NAME_BYTES,
            "metadata.authors",
        )?;
    }
    metadata.authors.sort();
    if metadata.authors.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(FoldTechniqueFileError::DuplicateIdentifier {
            kind: "metadata.author",
        });
    }
    match &metadata.source {
        FoldTechniqueSourceV1::UserAuthored => {}
        FoldTechniqueSourceV1::Adapted { citation_text }
        | FoldTechniqueSourceV1::PublishedReference { citation_text } => validate_text(
            citation_text,
            MAX_SOURCE_CITATION_CHARS,
            MAX_SOURCE_CITATION_BYTES,
            "metadata.source.citation_text",
        )?,
    }
    validate_spdx_identifier(&metadata.license_spdx_id)
}

fn validate_technique(
    technique: &mut FoldTechniqueTemplateV1,
) -> Result<(), FoldTechniqueFileError> {
    if technique.version == 0 || technique.version > MAX_TECHNIQUE_VERSION {
        return Err(FoldTechniqueFileError::InvalidField {
            field: "technique.version",
        });
    }
    validate_localized_texts(
        &mut technique.names,
        MAX_NAME_CHARS,
        MAX_NAME_BYTES,
        "technique.names",
    )?;
    validate_localized_texts(
        &mut technique.descriptions,
        MAX_DESCRIPTION_CHARS,
        MAX_DESCRIPTION_BYTES,
        "technique.descriptions",
    )?;
    if technique.parameters.len() > MAX_PARAMETERS {
        return Err(FoldTechniqueFileError::ResourceLimitExceeded {
            resource: "technique.parameters",
        });
    }
    if technique.preconditions.len() > MAX_PRECONDITIONS {
        return Err(FoldTechniqueFileError::ResourceLimitExceeded {
            resource: "technique.preconditions",
        });
    }
    if !(2..=MAX_OPERATIONS).contains(&technique.operations.len()) {
        return Err(FoldTechniqueFileError::ResourceLimitExceeded {
            resource: "technique.operations",
        });
    }

    let mut parameter_ids = HashSet::with_capacity(technique.parameters.len());
    for parameter in &mut technique.parameters {
        validate_parameter(parameter)?;
        if !parameter_ids.insert(parameter.id.clone()) {
            return Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "parameter.id",
            });
        }
    }
    technique
        .parameters
        .sort_by(|left, right| left.id.cmp(&right.id));
    let parameters = technique
        .parameters
        .iter()
        .map(|parameter| (parameter.id.as_str(), &parameter.parameter_type))
        .collect::<HashMap<_, _>>();

    let mut precondition_ids = HashSet::with_capacity(technique.preconditions.len());
    let mut total_nodes = 0_usize;
    for precondition in &mut technique.preconditions {
        validate_identifier(&precondition.id, MAX_IDENTIFIER_BYTES, "precondition.id")?;
        if !precondition_ids.insert(precondition.id.clone()) {
            return Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "precondition.id",
            });
        }
        validate_precondition(
            &mut precondition.condition,
            &parameters,
            1,
            &mut total_nodes,
        )?;
    }
    technique
        .preconditions
        .sort_by(|left, right| left.id.cmp(&right.id));

    let mut operation_ids = HashSet::with_capacity(technique.operations.len());
    for operation in &mut technique.operations {
        validate_operation(operation, &parameters, &precondition_ids)?;
        if !operation_ids.insert(operation.id.clone()) {
            return Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "operation.id",
            });
        }
    }
    Ok(())
}

fn validate_parameter(
    parameter: &mut FoldTechniqueParameterDefinitionV1,
) -> Result<(), FoldTechniqueFileError> {
    validate_identifier(&parameter.id, MAX_IDENTIFIER_BYTES, "parameter.id")?;
    validate_localized_texts(
        &mut parameter.names,
        MAX_NAME_CHARS,
        MAX_NAME_BYTES,
        "parameter.names",
    )?;
    validate_localized_texts(
        &mut parameter.descriptions,
        MAX_DESCRIPTION_CHARS,
        MAX_DESCRIPTION_BYTES,
        "parameter.descriptions",
    )?;
    match &mut parameter.parameter_type {
        FoldTechniqueParameterTypeV1::LengthMicrometres {
            minimum,
            maximum,
            default,
        } => validate_numeric_range(
            *minimum,
            *maximum,
            *default,
            0,
            MAX_LENGTH_MICROMETRES,
            "parameter.length_micrometres",
        ),
        FoldTechniqueParameterTypeV1::AngleMicrodegrees {
            minimum,
            maximum,
            default,
        } => validate_numeric_range(
            *minimum,
            *maximum,
            *default,
            -MAX_ABSOLUTE_ANGLE_MICRODEGREES,
            MAX_ABSOLUTE_ANGLE_MICRODEGREES,
            "parameter.angle_microdegrees",
        ),
        FoldTechniqueParameterTypeV1::RatioMillionths {
            minimum,
            maximum,
            default,
        } => validate_numeric_range(
            *minimum,
            *maximum,
            *default,
            1,
            MAX_RATIO_MILLIONTHS,
            "parameter.ratio_millionths",
        ),
        FoldTechniqueParameterTypeV1::Integer {
            minimum,
            maximum,
            default,
        } => validate_numeric_range(
            *minimum,
            *maximum,
            *default,
            -MAX_ABSOLUTE_INTEGER,
            MAX_ABSOLUTE_INTEGER,
            "parameter.integer",
        ),
        FoldTechniqueParameterTypeV1::Boolean { .. } => Ok(()),
        FoldTechniqueParameterTypeV1::Choice {
            options,
            default_option_id,
        } => {
            ensure_nonempty_limit(options.len(), MAX_CHOICES, "parameter.choice.options")?;
            let mut option_ids = HashSet::with_capacity(options.len());
            for option in options {
                validate_identifier(
                    &option.id,
                    MAX_IDENTIFIER_BYTES,
                    "parameter.choice.option.id",
                )?;
                if !option_ids.insert(option.id.clone()) {
                    return Err(FoldTechniqueFileError::DuplicateIdentifier {
                        kind: "parameter.choice.option.id",
                    });
                }
                validate_localized_texts(
                    &mut option.names,
                    MAX_NAME_CHARS,
                    MAX_NAME_BYTES,
                    "parameter.choice.option.names",
                )?;
            }
            validate_identifier(
                default_option_id,
                MAX_IDENTIFIER_BYTES,
                "parameter.choice.default_option_id",
            )?;
            if !option_ids.contains(default_option_id.as_str()) {
                return Err(FoldTechniqueFileError::MissingReference {
                    kind: "parameter.choice.default_option_id",
                });
            }
            Ok(())
        }
    }
}

fn validate_numeric_range(
    minimum: i64,
    maximum: i64,
    default: i64,
    allowed_minimum: i64,
    allowed_maximum: i64,
    field: &'static str,
) -> Result<(), FoldTechniqueFileError> {
    if minimum < allowed_minimum
        || maximum > allowed_maximum
        || minimum > maximum
        || default < minimum
        || default > maximum
    {
        Err(FoldTechniqueFileError::InvalidField { field })
    } else {
        Ok(())
    }
}

fn validate_precondition(
    condition: &mut FoldTechniquePreconditionV1,
    parameters: &HashMap<&str, &FoldTechniqueParameterTypeV1>,
    depth: usize,
    total_nodes: &mut usize,
) -> Result<(), FoldTechniqueFileError> {
    if depth > MAX_PRECONDITION_DEPTH {
        return Err(FoldTechniqueFileError::ResourceLimitExceeded {
            resource: "precondition.depth",
        });
    }
    *total_nodes =
        total_nodes
            .checked_add(1)
            .ok_or(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "precondition.nodes",
            })?;
    if *total_nodes > MAX_PRECONDITION_NODES_PER_TECHNIQUE {
        return Err(FoldTechniqueFileError::ResourceLimitExceeded {
            resource: "precondition.nodes",
        });
    }
    match condition {
        FoldTechniquePreconditionV1::All { conditions }
        | FoldTechniquePreconditionV1::Any { conditions } => {
            ensure_nonempty_limit(
                conditions.len(),
                MAX_PRECONDITION_NODES_PER_TECHNIQUE,
                "precondition.conditions",
            )?;
            for child in conditions {
                validate_precondition(child, parameters, depth + 1, total_nodes)?;
            }
            Ok(())
        }
        FoldTechniquePreconditionV1::Not { condition } => {
            validate_precondition(condition, parameters, depth + 1, total_nodes)
        }
        FoldTechniquePreconditionV1::ParameterComparison {
            parameter_id,
            comparison,
            value,
        } => {
            validate_identifier(
                parameter_id,
                MAX_IDENTIFIER_BYTES,
                "precondition.parameter_id",
            )?;
            let parameter_type = parameters.get(parameter_id.as_str()).ok_or(
                FoldTechniqueFileError::MissingReference {
                    kind: "precondition.parameter_id",
                },
            )?;
            validate_literal(parameter_type, *comparison, value)
        }
        FoldTechniquePreconditionV1::CapabilityAvailable { .. } => Ok(()),
        FoldTechniquePreconditionV1::UserConfirmation { prompts } => validate_localized_texts(
            prompts,
            MAX_DESCRIPTION_CHARS,
            MAX_DESCRIPTION_BYTES,
            "precondition.user_confirmation.prompts",
        ),
    }
}

fn validate_literal(
    parameter_type: &FoldTechniqueParameterTypeV1,
    comparison: FoldTechniqueComparisonV1,
    literal: &FoldTechniqueParameterLiteralV1,
) -> Result<(), FoldTechniqueFileError> {
    let in_range = match (parameter_type, literal) {
        (
            FoldTechniqueParameterTypeV1::LengthMicrometres {
                minimum, maximum, ..
            },
            FoldTechniqueParameterLiteralV1::LengthMicrometres { value },
        )
        | (
            FoldTechniqueParameterTypeV1::AngleMicrodegrees {
                minimum, maximum, ..
            },
            FoldTechniqueParameterLiteralV1::AngleMicrodegrees { value },
        )
        | (
            FoldTechniqueParameterTypeV1::RatioMillionths {
                minimum, maximum, ..
            },
            FoldTechniqueParameterLiteralV1::RatioMillionths { value },
        )
        | (
            FoldTechniqueParameterTypeV1::Integer {
                minimum, maximum, ..
            },
            FoldTechniqueParameterLiteralV1::Integer { value },
        ) => *value >= *minimum && *value <= *maximum,
        (
            FoldTechniqueParameterTypeV1::Boolean { .. },
            FoldTechniqueParameterLiteralV1::Boolean { .. },
        ) => matches!(
            comparison,
            FoldTechniqueComparisonV1::Equal | FoldTechniqueComparisonV1::NotEqual
        ),
        (
            FoldTechniqueParameterTypeV1::Choice { options, .. },
            FoldTechniqueParameterLiteralV1::Choice { option_id },
        ) => {
            matches!(
                comparison,
                FoldTechniqueComparisonV1::Equal | FoldTechniqueComparisonV1::NotEqual
            ) && options.iter().any(|option| option.id == *option_id)
        }
        _ => false,
    };
    if in_range {
        Ok(())
    } else {
        Err(FoldTechniqueFileError::ParameterTypeMismatch)
    }
}

fn validate_operation(
    operation: &mut FoldTechniqueOperationV1,
    parameters: &HashMap<&str, &FoldTechniqueParameterTypeV1>,
    precondition_ids: &HashSet<String>,
) -> Result<(), FoldTechniqueFileError> {
    validate_identifier(&operation.id, MAX_IDENTIFIER_BYTES, "operation.id")?;
    validate_localized_texts(
        &mut operation.names,
        MAX_NAME_CHARS,
        MAX_NAME_BYTES,
        "operation.names",
    )?;
    match &mut operation.action {
        FoldTechniqueActionV1::InstructionCue { instructions }
        | FoldTechniqueActionV1::LayerSelectiveManipulation { instructions } => {
            validate_localized_texts(
                instructions,
                MAX_DESCRIPTION_CHARS,
                MAX_DESCRIPTION_BYTES,
                "operation.action.instructions",
            )?;
        }
        FoldTechniqueActionV1::StraightLineStackedFold
        | FoldTechniqueActionV1::InsideReverseFold
        | FoldTechniqueActionV1::OutsideReverseFold
        | FoldTechniqueActionV1::SinkFold { .. } => {}
    }

    if operation.parameter_bindings.len() > MAX_OPERATION_PARAMETER_BINDINGS {
        return Err(FoldTechniqueFileError::ResourceLimitExceeded {
            resource: "operation.parameter_bindings",
        });
    }
    let mut roles = HashSet::with_capacity(operation.parameter_bindings.len());
    for binding in &operation.parameter_bindings {
        validate_identifier(
            &binding.role,
            MAX_IDENTIFIER_BYTES,
            "operation.parameter_binding.role",
        )?;
        validate_identifier(
            &binding.parameter_id,
            MAX_IDENTIFIER_BYTES,
            "operation.parameter_binding.parameter_id",
        )?;
        if !roles.insert(binding.role.clone()) {
            return Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "operation.parameter_binding.role",
            });
        }
        if !parameters.contains_key(binding.parameter_id.as_str()) {
            return Err(FoldTechniqueFileError::MissingReference {
                kind: "operation.parameter_binding.parameter_id",
            });
        }
    }
    operation
        .parameter_bindings
        .sort_by(|left, right| left.role.cmp(&right.role));

    if operation.precondition_ids.len() > MAX_OPERATION_PRECONDITIONS {
        return Err(FoldTechniqueFileError::ResourceLimitExceeded {
            resource: "operation.precondition_ids",
        });
    }
    let mut operation_preconditions = HashSet::with_capacity(operation.precondition_ids.len());
    for id in &operation.precondition_ids {
        validate_identifier(id, MAX_IDENTIFIER_BYTES, "operation.precondition_id")?;
        if !operation_preconditions.insert(id.clone()) {
            return Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "operation.precondition_id",
            });
        }
        if !precondition_ids.contains(id.as_str()) {
            return Err(FoldTechniqueFileError::MissingReference {
                kind: "operation.precondition_id",
            });
        }
    }
    operation.precondition_ids.sort();

    ensure_nonempty_limit(
        operation.required_capabilities.len(),
        MAX_OPERATION_CAPABILITIES,
        "operation.required_capabilities",
    )?;
    operation.required_capabilities.sort_unstable();
    if operation
        .required_capabilities
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(FoldTechniqueFileError::DuplicateIdentifier {
            kind: "operation.required_capability",
        });
    }
    validate_execution_support(operation)
}

fn validate_execution_support(
    operation: &FoldTechniqueOperationV1,
) -> Result<(), FoldTechniqueFileError> {
    let (required, expected_support) = match operation.action {
        FoldTechniqueActionV1::InstructionCue { .. } => (
            FoldTechniqueCapabilityV1::HumanInterpretationV1,
            FoldTechniqueExecutionSupportV1::DeclarativeOnly,
        ),
        FoldTechniqueActionV1::StraightLineStackedFold => (
            FoldTechniqueCapabilityV1::StraightLineStackedFoldV1,
            FoldTechniqueExecutionSupportV1::DeclarativeOnly,
        ),
        FoldTechniqueActionV1::InsideReverseFold => (
            FoldTechniqueCapabilityV1::InsideReverseFoldMotionV1,
            FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                operation: FoldTechniqueUnsupportedPhysicalOperationV1::InsideReverseFoldMotionV1,
            },
        ),
        FoldTechniqueActionV1::OutsideReverseFold => (
            FoldTechniqueCapabilityV1::OutsideReverseFoldMotionV1,
            FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                operation: FoldTechniqueUnsupportedPhysicalOperationV1::OutsideReverseFoldMotionV1,
            },
        ),
        FoldTechniqueActionV1::SinkFold { .. } => (
            FoldTechniqueCapabilityV1::SinkFoldMotionV1,
            FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                operation: FoldTechniqueUnsupportedPhysicalOperationV1::SinkFoldMotionV1,
            },
        ),
        FoldTechniqueActionV1::LayerSelectiveManipulation { .. } => (
            FoldTechniqueCapabilityV1::LayerSelectiveMotionV1,
            FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                operation: FoldTechniqueUnsupportedPhysicalOperationV1::LayerSelectiveMotionV1,
            },
        ),
    };
    if operation.execution_support != expected_support
        || !operation.required_capabilities.contains(&required)
        || operation
            .required_capabilities
            .iter()
            .copied()
            .filter(|capability| is_unsupported_physical_capability(*capability))
            .any(|capability| capability != required)
    {
        Err(FoldTechniqueFileError::InconsistentExecutionSupport)
    } else {
        Ok(())
    }
}

const fn is_unsupported_physical_capability(capability: FoldTechniqueCapabilityV1) -> bool {
    matches!(
        capability,
        FoldTechniqueCapabilityV1::LayerSelectiveMotionV1
            | FoldTechniqueCapabilityV1::InsideReverseFoldMotionV1
            | FoldTechniqueCapabilityV1::OutsideReverseFoldMotionV1
            | FoldTechniqueCapabilityV1::SinkFoldMotionV1
    )
}

fn validate_localized_texts(
    entries: &mut [FoldTechniqueLocalizedTextV1],
    max_chars: usize,
    max_bytes: usize,
    field: &'static str,
) -> Result<(), FoldTechniqueFileError> {
    ensure_nonempty_limit(entries.len(), MAX_LOCALES, field)?;
    let mut locales = HashSet::with_capacity(entries.len());
    for entry in entries.iter() {
        validate_locale(&entry.locale)?;
        validate_text(&entry.text, max_chars, max_bytes, field)?;
        if !locales.insert(entry.locale.clone()) {
            return Err(FoldTechniqueFileError::DuplicateIdentifier { kind: "locale" });
        }
    }
    entries.sort_by(|left, right| left.locale.cmp(&right.locale));
    Ok(())
}

fn validate_locale(locale: &str) -> Result<(), FoldTechniqueFileError> {
    if locale.is_empty()
        || locale.len() > MAX_LOCALE_BYTES
        || !locale.is_ascii()
        || locale.starts_with('-')
        || locale.ends_with('-')
    {
        return Err(FoldTechniqueFileError::InvalidField { field: "locale" });
    }
    let mut segments = locale.split('-');
    let first = segments.next().unwrap_or_default();
    if !(2..=8).contains(&first.len()) || !first.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return Err(FoldTechniqueFileError::InvalidField { field: "locale" });
    }
    if segments.any(|segment| {
        segment.is_empty()
            || segment.len() > 8
            || !segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    }) {
        return Err(FoldTechniqueFileError::InvalidField { field: "locale" });
    }
    Ok(())
}

fn validate_identifier(
    value: &str,
    maximum_bytes: usize,
    field: &'static str,
) -> Result<(), FoldTechniqueFileError> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || !value.is_ascii()
        || !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        || !value
            .as_bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(FoldTechniqueFileError::InvalidField { field });
    }
    let mut separator = false;
    for byte in value.bytes() {
        let current_separator = matches!(byte, b'.' | b'-' | b'_');
        if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || current_separator)
            || (separator && current_separator)
        {
            return Err(FoldTechniqueFileError::InvalidField { field });
        }
        separator = current_separator;
    }
    Ok(())
}

fn validate_spdx_identifier(value: &str) -> Result<(), FoldTechniqueFileError> {
    if value.is_empty()
        || value.len() > MAX_LICENSE_ID_BYTES
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
    {
        Err(FoldTechniqueFileError::InvalidField {
            field: "metadata.license_spdx_id",
        })
    } else {
        Ok(())
    }
}

fn validate_text(
    value: &str,
    maximum_chars: usize,
    maximum_bytes: usize,
    field: &'static str,
) -> Result<(), FoldTechniqueFileError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > maximum_bytes
        || value.chars().count() > maximum_chars
        || value.chars().any(is_disallowed_text_character)
    {
        Err(FoldTechniqueFileError::InvalidField { field })
    } else {
        Ok(())
    }
}

fn is_disallowed_text_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{200e}'
                | '\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn ensure_nonempty_limit(
    actual: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), FoldTechniqueFileError> {
    if actual == 0 || actual > maximum {
        Err(FoldTechniqueFileError::ResourceLimitExceeded { resource })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    fn localized(ja: &str, en: &str) -> Vec<FoldTechniqueLocalizedTextV1> {
        vec![
            FoldTechniqueLocalizedTextV1 {
                locale: "ja".to_owned(),
                text: ja.to_owned(),
            },
            FoldTechniqueLocalizedTextV1 {
                locale: "en".to_owned(),
                text: en.to_owned(),
            },
        ]
    }

    fn angle_parameter() -> FoldTechniqueParameterDefinitionV1 {
        FoldTechniqueParameterDefinitionV1 {
            id: "target_angle".to_owned(),
            names: localized("目標角度", "Target angle"),
            descriptions: localized("折り動作の説明用角度", "Descriptive fold angle"),
            parameter_type: FoldTechniqueParameterTypeV1::AngleMicrodegrees {
                minimum: -180_000_000,
                maximum: 180_000_000,
                default: 180_000_000,
            },
        }
    }

    fn confirmation() -> FoldTechniquePreconditionDefinitionV1 {
        FoldTechniquePreconditionDefinitionV1 {
            id: "confirm_layers".to_owned(),
            condition: FoldTechniquePreconditionV1::All {
                conditions: vec![
                    FoldTechniquePreconditionV1::UserConfirmation {
                        prompts: localized(
                            "層の位置を目視で確認する",
                            "Visually confirm the layer positions",
                        ),
                    },
                    FoldTechniquePreconditionV1::ParameterComparison {
                        parameter_id: "target_angle".to_owned(),
                        comparison: FoldTechniqueComparisonV1::GreaterThan,
                        value: FoldTechniqueParameterLiteralV1::AngleMicrodegrees { value: 0 },
                    },
                ],
            },
        }
    }

    fn cue(id: &str, ja: &str, en: &str) -> FoldTechniqueOperationV1 {
        FoldTechniqueOperationV1 {
            id: id.to_owned(),
            names: localized(ja, en),
            action: FoldTechniqueActionV1::InstructionCue {
                instructions: localized(ja, en),
            },
            parameter_bindings: Vec::new(),
            precondition_ids: Vec::new(),
            required_capabilities: vec![
                FoldTechniqueCapabilityV1::InstructionTimelineV1,
                FoldTechniqueCapabilityV1::HumanInterpretationV1,
            ],
            execution_support: FoldTechniqueExecutionSupportV1::DeclarativeOnly,
        }
    }

    fn physical_operation(
        id: &str,
        ja: &str,
        en: &str,
        action: FoldTechniqueActionV1,
        capability: FoldTechniqueCapabilityV1,
        unsupported: FoldTechniqueUnsupportedPhysicalOperationV1,
    ) -> FoldTechniqueOperationV1 {
        FoldTechniqueOperationV1 {
            id: id.to_owned(),
            names: localized(ja, en),
            action,
            parameter_bindings: vec![FoldTechniqueParameterBindingV1 {
                role: "target_angle".to_owned(),
                parameter_id: "target_angle".to_owned(),
            }],
            precondition_ids: vec!["confirm_layers".to_owned()],
            required_capabilities: vec![
                FoldTechniqueCapabilityV1::InstructionTimelineV1,
                capability,
            ],
            execution_support: FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                operation: unsupported,
            },
        }
    }

    fn technique(
        id: &str,
        ja: &str,
        en: &str,
        physical: FoldTechniqueOperationV1,
    ) -> FoldTechniqueTemplateV1 {
        FoldTechniqueTemplateV1 {
            id: id.to_owned(),
            version: 1,
            names: localized(ja, en),
            descriptions: localized(
                "共有用の宣言的技法。自動実行可能性を表さない。",
                "Declarative shared technique; it does not assert executability.",
            ),
            parameters: vec![angle_parameter()],
            preconditions: vec![confirmation()],
            operations: vec![
                cue("prepare", "準備", "Prepare"),
                physical,
                cue("verify", "形を確認", "Verify the shape"),
            ],
        }
    }

    fn complete_document() -> FoldTechniqueFileDocumentV1 {
        FoldTechniqueFileDocumentV1 {
            schema: FOLD_TECHNIQUE_FILE_SCHEMA_V1.to_owned(),
            version: FOLD_TECHNIQUE_FILE_VERSION_V1,
            package_id: "user.example.classic-techniques".to_owned(),
            metadata: FoldTechniqueMetadataV1 {
                authors: vec!["Example Author".to_owned()],
                source: FoldTechniqueSourceV1::UserAuthored,
                license_spdx_id: "CC-BY-4.0".to_owned(),
            },
            techniques: vec![
                technique(
                    "user.example.inside-reverse",
                    "中割り折り",
                    "Inside reverse fold",
                    physical_operation(
                        "reverse",
                        "層を内側へ反転",
                        "Reverse selected layers inward",
                        FoldTechniqueActionV1::InsideReverseFold,
                        FoldTechniqueCapabilityV1::InsideReverseFoldMotionV1,
                        FoldTechniqueUnsupportedPhysicalOperationV1::InsideReverseFoldMotionV1,
                    ),
                ),
                technique(
                    "user.example.outside-reverse",
                    "かぶせ折り",
                    "Outside reverse fold",
                    physical_operation(
                        "reverse",
                        "層を外側へ反転",
                        "Reverse selected layers outward",
                        FoldTechniqueActionV1::OutsideReverseFold,
                        FoldTechniqueCapabilityV1::OutsideReverseFoldMotionV1,
                        FoldTechniqueUnsupportedPhysicalOperationV1::OutsideReverseFoldMotionV1,
                    ),
                ),
                technique(
                    "user.example.open-sink",
                    "沈め折り",
                    "Open sink fold",
                    physical_operation(
                        "sink",
                        "指定部分を沈める",
                        "Sink the indicated region",
                        FoldTechniqueActionV1::SinkFold {
                            sink_kind: FoldTechniqueSinkKindV1::Open,
                        },
                        FoldTechniqueCapabilityV1::SinkFoldMotionV1,
                        FoldTechniqueUnsupportedPhysicalOperationV1::SinkFoldMotionV1,
                    ),
                ),
            ],
        }
    }

    fn valid_bytes() -> Vec<u8> {
        let validated =
            validate_fold_technique_file_v1(complete_document()).expect("valid fixture");
        write_fold_technique_file_v1(&validated).expect("deterministic bytes")
    }

    #[test]
    fn inside_outside_and_sink_are_inert_explicitly_unsupported_metadata() {
        let file = validate_fold_technique_file_v1(complete_document()).expect("fixture");
        assert!(!file.grants_project_mutation_authority());
        assert!(!file.permits_code_execution());
        assert!(!file.permits_external_resource_access());
        assert_eq!(file.document().techniques.len(), 3);
        for technique in &file.document().techniques {
            let physical = &technique.operations[1];
            assert!(matches!(
                physical.execution_support,
                FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation { .. }
            ));
        }
    }

    #[test]
    fn canonical_json_is_deterministic_and_passes_independent_read_back() {
        let mut first = complete_document();
        let mut second = first.clone();
        first.techniques.reverse();
        second.metadata.authors.reverse();
        for technique in &mut second.techniques {
            technique.names.reverse();
            technique.descriptions.reverse();
            technique.parameters.reverse();
            technique.preconditions.reverse();
            for operation in &mut technique.operations {
                operation.names.reverse();
                operation.required_capabilities.reverse();
                operation.parameter_bindings.reverse();
                operation.precondition_ids.reverse();
                if let FoldTechniqueActionV1::InstructionCue { instructions }
                | FoldTechniqueActionV1::LayerSelectiveManipulation { instructions } =
                    &mut operation.action
                {
                    instructions.reverse();
                }
            }
        }
        let first = validate_fold_technique_file_v1(first).expect("first");
        let second = validate_fold_technique_file_v1(second).expect("second");
        let first_bytes = write_fold_technique_file_v1(&first).expect("first bytes");
        let second_bytes = write_fold_technique_file_v1(&second).expect("second bytes");
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            read_fold_technique_file_v1(&first_bytes).expect("read-back"),
            first
        );
        let digest = format!("{:x}", Sha256::digest(&first_bytes));
        assert_eq!(
            digest,
            "d27acd4ceceafdee1d2a3f6a0b4edf472842f0687b9f99f15365290fcfc25a45"
        );
    }

    #[test]
    fn unknown_code_path_url_and_mutation_fields_are_rejected() {
        let baseline: Value = serde_json::from_slice(&valid_bytes()).expect("fixture JSON");
        for (pointer, field) in [
            ("", "script"),
            ("/metadata", "path"),
            ("/techniques/0", "url"),
            ("/techniques/0/operations/0", "project_command"),
            ("/techniques/0/operations/0/action", "code"),
        ] {
            let mut hostile = baseline.clone();
            hostile
                .pointer_mut(pointer)
                .and_then(Value::as_object_mut)
                .expect("object")
                .insert(field.to_owned(), json!("hostile"));
            assert_eq!(
                read_fold_technique_file_v1(&serde_json::to_vec(&hostile).expect("hostile JSON")),
                Err(FoldTechniqueFileError::InvalidJson),
                "{pointer}/{field}"
            );
        }
    }

    #[test]
    fn expression_injection_has_no_admission_path() {
        let mut value: Value = serde_json::from_slice(&valid_bytes()).expect("fixture JSON");
        let condition = value
            .pointer_mut("/techniques/0/preconditions/0/condition/conditions/1")
            .and_then(Value::as_object_mut)
            .expect("comparison");
        condition.remove("value");
        condition.insert(
            "expression".to_owned(),
            json!("1 + run_script('mutate-project')"),
        );
        assert_eq!(
            read_fold_technique_file_v1(
                &serde_json::to_vec(&value).expect("expression injection JSON")
            ),
            Err(FoldTechniqueFileError::InvalidJson)
        );
    }

    #[test]
    fn every_shared_reference_and_duplicate_is_checked() {
        let baseline = complete_document();

        let mut duplicate_technique = baseline.clone();
        duplicate_technique.techniques[1].id = duplicate_technique.techniques[0].id.clone();
        assert!(matches!(
            validate_fold_technique_file_v1(duplicate_technique),
            Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "technique.id"
            })
        ));

        let mut duplicate_parameter = baseline.clone();
        let duplicate = duplicate_parameter.techniques[0].parameters[0].clone();
        duplicate_parameter.techniques[0].parameters.push(duplicate);
        assert!(matches!(
            validate_fold_technique_file_v1(duplicate_parameter),
            Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "parameter.id"
            })
        ));

        let mut missing_parameter = baseline.clone();
        missing_parameter.techniques[0].operations[1].parameter_bindings[0].parameter_id =
            "missing".to_owned();
        assert!(matches!(
            validate_fold_technique_file_v1(missing_parameter),
            Err(FoldTechniqueFileError::MissingReference {
                kind: "operation.parameter_binding.parameter_id"
            })
        ));

        let mut missing_precondition = baseline.clone();
        missing_precondition.techniques[0].operations[1].precondition_ids[0] = "missing".to_owned();
        assert!(matches!(
            validate_fold_technique_file_v1(missing_precondition),
            Err(FoldTechniqueFileError::MissingReference {
                kind: "operation.precondition_id"
            })
        ));

        let mut duplicate_operation = baseline;
        duplicate_operation.techniques[0].operations[2].id =
            duplicate_operation.techniques[0].operations[0].id.clone();
        assert!(matches!(
            validate_fold_technique_file_v1(duplicate_operation),
            Err(FoldTechniqueFileError::DuplicateIdentifier {
                kind: "operation.id"
            })
        ));
    }

    #[test]
    fn typed_literals_must_match_the_declared_type_range_and_comparison() {
        let mut wrong_type = complete_document();
        if let FoldTechniquePreconditionV1::All { conditions } =
            &mut wrong_type.techniques[0].preconditions[0].condition
            && let FoldTechniquePreconditionV1::ParameterComparison { value, .. } =
                &mut conditions[1]
        {
            *value = FoldTechniqueParameterLiteralV1::Integer { value: 1 };
        }
        assert_eq!(
            validate_fold_technique_file_v1(wrong_type),
            Err(FoldTechniqueFileError::ParameterTypeMismatch)
        );

        let mut out_of_range = complete_document();
        if let FoldTechniquePreconditionV1::All { conditions } =
            &mut out_of_range.techniques[0].preconditions[0].condition
            && let FoldTechniquePreconditionV1::ParameterComparison { value, .. } =
                &mut conditions[1]
        {
            *value = FoldTechniqueParameterLiteralV1::AngleMicrodegrees { value: 180_000_001 };
        }
        assert_eq!(
            validate_fold_technique_file_v1(out_of_range),
            Err(FoldTechniqueFileError::ParameterTypeMismatch)
        );
    }

    #[test]
    fn unsupported_physical_operation_cannot_be_downgraded_or_mislabeled() {
        for mutation in 0..4 {
            let mut document = complete_document();
            let operation = &mut document.techniques[0].operations[1];
            match mutation {
                0 => {
                    operation.execution_support = FoldTechniqueExecutionSupportV1::DeclarativeOnly;
                }
                1 => {
                    operation.execution_support =
                        FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                            operation:
                                FoldTechniqueUnsupportedPhysicalOperationV1::SinkFoldMotionV1,
                        };
                }
                2 => operation.required_capabilities.clear(),
                _ => operation
                    .required_capabilities
                    .push(FoldTechniqueCapabilityV1::SinkFoldMotionV1),
            }
            let error =
                validate_fold_technique_file_v1(document).expect_err("must reject downgrade");
            if mutation == 2 {
                assert_eq!(
                    error,
                    FoldTechniqueFileError::ResourceLimitExceeded {
                        resource: "operation.required_capabilities"
                    }
                );
            } else {
                assert_eq!(error, FoldTechniqueFileError::InconsistentExecutionSupport);
            }
        }
    }

    #[test]
    fn every_collection_has_a_fixed_non_relaxable_ceiling() {
        let mut authors = complete_document();
        authors.metadata.authors = (0..=MAX_AUTHORS)
            .map(|index| format!("Author {index}"))
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(authors),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "metadata.authors"
            })
        ));

        let mut locales = complete_document();
        locales.techniques[0].names = (0..=MAX_LOCALES)
            .map(|index| FoldTechniqueLocalizedTextV1 {
                locale: format!(
                    "{}{}",
                    char::from(b'a' + u8::try_from(index / 26).expect("small index")),
                    char::from(b'a' + u8::try_from(index % 26).expect("small index"))
                ),
                text: format!("Name {index}"),
            })
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(locales),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "technique.names"
            })
        ));

        let mut parameters = complete_document();
        let parameter = parameters.techniques[0].parameters[0].clone();
        parameters.techniques[0].parameters = (0..=MAX_PARAMETERS)
            .map(|index| FoldTechniqueParameterDefinitionV1 {
                id: format!("parameter-{index}"),
                ..parameter.clone()
            })
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(parameters),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "technique.parameters"
            })
        ));

        let mut choices = complete_document();
        choices.techniques[0].parameters[0].parameter_type = FoldTechniqueParameterTypeV1::Choice {
            options: (0..=MAX_CHOICES)
                .map(|index| FoldTechniqueChoiceOptionV1 {
                    id: format!("option-{index}"),
                    names: localized(&format!("選択肢{index}"), &format!("Option {index}")),
                })
                .collect(),
            default_option_id: "option-0".to_owned(),
        };
        assert!(matches!(
            validate_fold_technique_file_v1(choices),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "parameter.choice.options"
            })
        ));

        let mut preconditions = complete_document();
        let precondition = preconditions.techniques[0].preconditions[0].clone();
        preconditions.techniques[0].preconditions = (0..=MAX_PRECONDITIONS)
            .map(|index| FoldTechniquePreconditionDefinitionV1 {
                id: format!("precondition-{index}"),
                ..precondition.clone()
            })
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(preconditions),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "technique.preconditions"
            })
        ));

        let mut operations = complete_document();
        let operation = operations.techniques[0].operations[0].clone();
        operations.techniques[0].operations = (0..=MAX_OPERATIONS)
            .map(|index| FoldTechniqueOperationV1 {
                id: format!("operation-{index}"),
                ..operation.clone()
            })
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(operations),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "technique.operations"
            })
        ));

        let mut bindings = complete_document();
        bindings.techniques[0].operations[1].parameter_bindings = (0
            ..=MAX_OPERATION_PARAMETER_BINDINGS)
            .map(|index| FoldTechniqueParameterBindingV1 {
                role: format!("role-{index}"),
                parameter_id: "target_angle".to_owned(),
            })
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(bindings),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "operation.parameter_bindings"
            })
        ));

        let mut references = complete_document();
        references.techniques[0].operations[1].precondition_ids = (0..=MAX_OPERATION_PRECONDITIONS)
            .map(|index| format!("precondition-{index}"))
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(references),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "operation.precondition_ids"
            })
        ));

        let mut capabilities = complete_document();
        capabilities.techniques[0].operations[1].required_capabilities = vec![
            FoldTechniqueCapabilityV1::InsideReverseFoldMotionV1;
            MAX_OPERATION_CAPABILITIES + 1
        ];
        assert!(matches!(
            validate_fold_technique_file_v1(capabilities),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "operation.required_capabilities"
            })
        ));
    }

    #[test]
    fn caller_created_documents_are_also_bounded_by_canonical_encoded_bytes() {
        let mut document = complete_document();
        let template = document.techniques[0].clone();
        document.techniques = (0..MAX_TECHNIQUES)
            .map(|index| {
                let mut technique = FoldTechniqueTemplateV1 {
                    id: format!("user.large.technique-{index}"),
                    ..template.clone()
                };
                for description in &mut technique.descriptions {
                    description.text = "🟦".repeat(MAX_DESCRIPTION_CHARS);
                    assert_eq!(description.text.len(), MAX_DESCRIPTION_BYTES);
                }
                technique
            })
            .collect();
        assert_eq!(
            validate_fold_technique_file_v1(document),
            Err(FoldTechniqueFileError::InputTooLarge)
        );
    }

    #[test]
    fn semantic_depth_accepts_the_exact_boundary_and_rejects_one_more() {
        fn nested_not(depth: usize) -> FoldTechniquePreconditionV1 {
            let mut condition = FoldTechniquePreconditionV1::UserConfirmation {
                prompts: localized("確認", "Confirm"),
            };
            for _ in 1..depth {
                condition = FoldTechniquePreconditionV1::Not {
                    condition: Box::new(condition),
                };
            }
            condition
        }

        let mut exact = complete_document();
        exact.techniques[0].preconditions[0].condition = nested_not(MAX_PRECONDITION_DEPTH);
        validate_fold_technique_file_v1(exact).expect("exact depth boundary");

        let mut over = complete_document();
        over.techniques[0].preconditions[0].condition = nested_not(MAX_PRECONDITION_DEPTH + 1);
        assert!(matches!(
            validate_fold_technique_file_v1(over),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "precondition.depth"
            })
        ));
    }

    #[test]
    fn metadata_locale_choice_and_boolean_contracts_fail_closed() {
        let mut invalid_locale = complete_document();
        invalid_locale.techniques[0].names[0].locale = "ja-JP".to_owned();
        assert!(matches!(
            validate_fold_technique_file_v1(invalid_locale),
            Err(FoldTechniqueFileError::InvalidField { field: "locale" })
        ));

        let mut invalid_license = complete_document();
        invalid_license.metadata.license_spdx_id = "GPL-3.0 OR MIT".to_owned();
        assert!(matches!(
            validate_fold_technique_file_v1(invalid_license),
            Err(FoldTechniqueFileError::InvalidField {
                field: "metadata.license_spdx_id"
            })
        ));

        let mut dangling_choice = complete_document();
        dangling_choice.techniques[0].parameters[0].parameter_type =
            FoldTechniqueParameterTypeV1::Choice {
                options: vec![FoldTechniqueChoiceOptionV1 {
                    id: "one".to_owned(),
                    names: localized("一", "One"),
                }],
                default_option_id: "missing".to_owned(),
            };
        assert!(matches!(
            validate_fold_technique_file_v1(dangling_choice),
            Err(FoldTechniqueFileError::MissingReference {
                kind: "parameter.choice.default_option_id"
            })
        ));

        let mut boolean_ordering = complete_document();
        boolean_ordering.techniques[0].parameters[0].parameter_type =
            FoldTechniqueParameterTypeV1::Boolean { default: false };
        if let FoldTechniquePreconditionV1::All { conditions } =
            &mut boolean_ordering.techniques[0].preconditions[0].condition
            && let FoldTechniquePreconditionV1::ParameterComparison {
                comparison, value, ..
            } = &mut conditions[1]
        {
            *comparison = FoldTechniqueComparisonV1::GreaterThan;
            *value = FoldTechniqueParameterLiteralV1::Boolean { value: true };
        }
        assert_eq!(
            validate_fold_technique_file_v1(boolean_ordering),
            Err(FoldTechniqueFileError::ParameterTypeMismatch)
        );
    }

    #[test]
    fn encoded_bytes_structural_depth_and_semantic_depth_are_bounded() {
        let oversized = vec![b' '; MAX_FOLD_TECHNIQUE_FILE_BYTES + 1];
        assert_eq!(
            read_fold_technique_file_v1(&oversized),
            Err(FoldTechniqueFileError::InputTooLarge)
        );

        let deeply_nested = format!(
            "{}0{}",
            "[".repeat(MAX_JSON_STRUCTURAL_DEPTH + 1),
            "]".repeat(MAX_JSON_STRUCTURAL_DEPTH + 1)
        );
        assert_eq!(
            read_fold_technique_file_v1(deeply_nested.as_bytes()),
            Err(FoldTechniqueFileError::JsonNestingTooDeep)
        );

        let mut semantic = FoldTechniquePreconditionV1::UserConfirmation {
            prompts: localized("確認", "Confirm"),
        };
        for _ in 0..MAX_PRECONDITION_DEPTH {
            semantic = FoldTechniquePreconditionV1::Not {
                condition: Box::new(semantic),
            };
        }
        let mut document = complete_document();
        document.techniques[0].preconditions[0].condition = semantic;
        assert!(matches!(
            validate_fold_technique_file_v1(document),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "precondition.depth"
            })
        ));
    }

    #[test]
    fn count_character_numeric_and_identifier_limits_are_non_relaxable() {
        let mut too_many = complete_document();
        let template = too_many.techniques[0].clone();
        too_many.techniques = (0..=MAX_TECHNIQUES)
            .map(|index| FoldTechniqueTemplateV1 {
                id: format!("user.example.technique-{index}"),
                ..template.clone()
            })
            .collect();
        assert!(matches!(
            validate_fold_technique_file_v1(too_many),
            Err(FoldTechniqueFileError::ResourceLimitExceeded {
                resource: "techniques"
            })
        ));

        let mut long_text = complete_document();
        long_text.techniques[0].names[0].text = "あ".repeat(MAX_NAME_CHARS + 1);
        assert!(matches!(
            validate_fold_technique_file_v1(long_text),
            Err(FoldTechniqueFileError::InvalidField {
                field: "technique.names"
            })
        ));

        let mut numeric = complete_document();
        numeric.techniques[0].parameters[0].parameter_type =
            FoldTechniqueParameterTypeV1::AngleMicrodegrees {
                minimum: -180_000_000,
                maximum: 180_000_001,
                default: 0,
            };
        assert!(matches!(
            validate_fold_technique_file_v1(numeric),
            Err(FoldTechniqueFileError::InvalidField {
                field: "parameter.angle_microdegrees"
            })
        ));

        let mut traversal = complete_document();
        traversal.techniques[0].id = "../execute".to_owned();
        assert!(matches!(
            validate_fold_technique_file_v1(traversal),
            Err(FoldTechniqueFileError::InvalidField {
                field: "technique.id"
            })
        ));
    }

    #[test]
    fn duplicate_json_keys_and_unknown_enum_values_are_rejected() {
        let bytes = valid_bytes();
        let text = String::from_utf8(bytes).expect("UTF-8 fixture");
        let duplicate = text.replacen(
            "\"schema\":\"origami2_fold_technique_file\"",
            "\"schema\":\"origami2_fold_technique_file\",\"schema\":\"origami2_fold_technique_file\"",
            1,
        );
        assert_eq!(
            read_fold_technique_file_v1(duplicate.as_bytes()),
            Err(FoldTechniqueFileError::InvalidJson)
        );

        let mut unknown: Value = serde_json::from_str(&text).expect("fixture");
        unknown["techniques"][0]["operations"][0]["required_capabilities"][0] =
            json!("execute_arbitrary_code_v1");
        assert_eq!(
            read_fold_technique_file_v1(&serde_json::to_vec(&unknown).expect("unknown enum JSON")),
            Err(FoldTechniqueFileError::InvalidJson)
        );
    }

    #[test]
    fn generated_round_trip_property_holds_for_many_bounded_user_files() {
        for seed in 0_u32..256 {
            let mut document = complete_document();
            document.package_id = format!("user.generated.package-{seed}");
            document.metadata.authors = vec![format!("Author {seed}")];
            for (index, technique) in document.techniques.iter_mut().enumerate() {
                technique.version = seed % MAX_TECHNIQUE_VERSION + 1;
                technique.id = format!("user.generated.technique-{seed}-{index}");
                if seed & 1 == 1 {
                    technique.names.reverse();
                    technique.descriptions.reverse();
                    technique.parameters[0].names.reverse();
                }
                if seed & 2 == 2 {
                    technique.operations[0].required_capabilities.reverse();
                }
                if let FoldTechniqueParameterTypeV1::AngleMicrodegrees { default, .. } =
                    &mut technique.parameters[0].parameter_type
                {
                    *default = i64::from(seed % 181) * 1_000_000;
                }
            }
            if seed & 4 == 4 {
                document.techniques.reverse();
            }
            let validated =
                validate_fold_technique_file_v1(document).expect("generated valid file");
            let once = write_fold_technique_file_v1(&validated).expect("first write");
            let restored = read_fold_technique_file_v1(&once).expect("generated read");
            let twice = write_fold_technique_file_v1(&restored).expect("second write");
            assert_eq!(once, twice, "seed {seed}");
            assert_eq!(validated, restored, "seed {seed}");
        }
    }
}
