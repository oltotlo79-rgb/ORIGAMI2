//! Deterministic, bounded export of one static indexed triangle mesh.
//!
//! The admitted mesh uses millimetres and a right-handed, Z-up coordinate
//! system. OBJ and binary STL preserve those axes. GLB stores local positions
//! in metres and carries a fixed node rotation into glTF's Y-up scene axes.
//! This module deliberately has no project, current-pose, animation, material,
//! texture, staging, filesystem, or UI authority.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Version of the only indexed triangle mesh input schema accepted here.
pub const INDEXED_TRIANGLE_MESH_SCHEMA_VERSION_V1: u32 = 1;
/// Non-relaxable maximum number of indexed vertices.
pub const MAX_STATIC_MESH_VERTICES: usize = 100_000;
/// Non-relaxable maximum number of triangles.
pub const MAX_STATIC_MESH_TRIANGLES: usize = 200_000;
/// Non-relaxable maximum size of one exported file.
pub const MAX_STATIC_MESH_EXPORT_BYTES: usize = 64 * 1024 * 1024;
/// Non-relaxable mesh-name length in Unicode scalar values.
pub const MAX_STATIC_MESH_NAME_CHARS: usize = 120;
/// Non-relaxable mesh-name length in UTF-8 bytes.
pub const MAX_STATIC_MESH_NAME_BYTES: usize = 512;
/// Unit of every admitted position and of OBJ/STL coordinates.
pub const STATIC_MESH_SOURCE_UNIT: &str = "millimeter";
/// Common source axes. `X × Y = Z`.
pub const STATIC_MESH_SOURCE_AXIS: &str = "right-handed X-right Y-forward Z-up";

const OBJ_HEADER: &str = "# ORIGAMI2 static indexed triangle mesh";
const OBJ_UNIT: &str = "# unit: millimeter";
const OBJ_AXIS: &str = "# axis: right-handed X-right Y-forward Z-up";
const MAX_OBJ_LINE_BYTES: usize = 2_048;
const STL_HEADER_BYTES: usize = 80;
const STL_TRIANGLE_BYTES: usize = 50;
const GLB_HEADER_BYTES: usize = 12;
const GLB_CHUNK_HEADER_BYTES: usize = 8;
const GLB_JSON_CHUNK_TYPE: u32 = 0x4e4f_534a;
const GLB_BIN_CHUNK_TYPE: u32 = 0x004e_4942;
const MAX_GLB_JSON_BYTES: usize = 64 * 1024;
const GLTF_ARRAY_BUFFER: u32 = 34_962;
const GLTF_ELEMENT_ARRAY_BUFFER: u32 = 34_963;
const GLTF_FLOAT: u32 = 5_126;
const GLTF_UNSIGNED_INT: u32 = 5_125;
const GLTF_TRIANGLES: u32 = 4;
const GLTF_NODE_MATRIX: [f32; 16] = [
    -1.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, 1.0,
];

/// Versioned, untrusted input DTO for a static indexed triangle mesh.
///
/// `normals.len()` must equal `positions_mm.len()`. A normal belongs to the
/// vertex at the same index. Array order is semantic and is preserved by all
/// exporters; callers that need canonical source-independent order must choose
/// that order before admission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedTriangleMeshV1 {
    pub schema_version: u32,
    pub name: String,
    pub positions_mm: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
}

impl IndexedTriangleMeshV1 {
    /// Creates a V1 input DTO. Admission is still required before export.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        positions_mm: Vec<[f64; 3]>,
        normals: Vec<[f64; 3]>,
        triangles: Vec<[u32; 3]>,
    ) -> Self {
        Self {
            schema_version: INDEXED_TRIANGLE_MESH_SCHEMA_VERSION_V1,
            name: name.into(),
            positions_mm,
            normals,
            triangles,
        }
    }
}

/// Immutable mesh capability returned only after complete bounded admission.
///
/// Coordinates and normals have canonical positive zero. Normals are
/// deterministically normalized. Private fields prevent bypassing admission.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedIndexedTriangleMesh {
    name: String,
    safe_ascii_name: String,
    positions_mm: Vec<[f64; 3]>,
    normals: Vec<[f64; 3]>,
    triangles: Vec<[u32; 3]>,
}

impl ValidatedIndexedTriangleMesh {
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        INDEXED_TRIANGLE_MESH_SCHEMA_VERSION_V1
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn positions_mm(&self) -> &[[f64; 3]] {
        &self.positions_mm
    }

    #[must_use]
    pub fn normals(&self) -> &[[f64; 3]] {
        &self.normals
    }

    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
}

/// Static 3D interchange format emitted by this foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StaticMeshExportFormat {
    /// Text OBJ with millimetre and axis comments, intended for Blender.
    #[serde(rename = "obj")]
    Obj,
    /// Little-endian binary STL with millimetre metadata, intended for slicers.
    #[serde(rename = "binary_stl")]
    BinaryStl,
    /// glTF 2.0 binary container with metre positions and indexed geometry.
    #[serde(rename = "glb")]
    Glb20,
}

impl StaticMeshExportFormat {
    #[must_use]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Obj => "model/obj",
            Self::BinaryStl => "model/stl",
            Self::Glb20 => "model/gltf-binary",
        }
    }

    #[must_use]
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::Obj => "obj",
            Self::BinaryStl => "stl",
            Self::Glb20 => "glb",
        }
    }
}

/// Limits used during admission and export.
///
/// Caller values can tighten, but can never relax, the public hard ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticMeshExportLimits {
    pub max_output_bytes: usize,
    pub max_vertices: usize,
    pub max_triangles: usize,
    pub max_name_chars: usize,
    pub max_name_bytes: usize,
}

impl Default for StaticMeshExportLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: MAX_STATIC_MESH_EXPORT_BYTES,
            max_vertices: MAX_STATIC_MESH_VERTICES,
            max_triangles: MAX_STATIC_MESH_TRIANGLES,
            max_name_chars: MAX_STATIC_MESH_NAME_CHARS,
            max_name_bytes: MAX_STATIC_MESH_NAME_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticMeshExportArtifact {
    pub format: StaticMeshExportFormat,
    pub media_type: &'static str,
    pub file_extension: &'static str,
    pub bytes: Vec<u8>,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticMeshEncodedPrecision {
    BinaryStlMillimetres,
    GlbMetres,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StaticMeshExportError {
    #[error("mesh schema version {found} is unsupported; latest supported version is {latest}")]
    UnsupportedSchemaVersion { found: u32, latest: u32 },
    #[error("mesh name has {actual} characters; the limit is {maximum}")]
    NameTooManyCharacters { actual: usize, maximum: usize },
    #[error("mesh name has {actual} UTF-8 bytes; the limit is {maximum}")]
    NameTooManyBytes { actual: usize, maximum: usize },
    #[error(
        "mesh name contains unsupported character U+{code_point:04X} at character {character_index}"
    )]
    InvalidNameCharacter {
        character_index: usize,
        code_point: u32,
    },
    #[error("mesh has no vertices")]
    NoVertices,
    #[error("mesh has no triangles")]
    NoTriangles,
    #[error("mesh has {actual} vertices; the limit is {maximum}")]
    TooManyVertices { actual: usize, maximum: usize },
    #[error("mesh has {actual} triangles; the limit is {maximum}")]
    TooManyTriangles { actual: usize, maximum: usize },
    #[error("mesh has {actual} normals; exactly {expected} are required")]
    NormalCountMismatch { actual: usize, expected: usize },
    #[error("mesh vertex {vertex_index} has a non-finite coordinate")]
    NonFinitePosition { vertex_index: usize },
    #[error("mesh vertex {vertex_index} has a non-finite normal")]
    NonFiniteNormal { vertex_index: usize },
    #[error("mesh vertex {vertex_index} has a zero or unrepresentable normal")]
    InvalidNormal { vertex_index: usize },
    #[error("mesh vertex {vertex_index} cannot be represented by {precision:?} finite coordinates")]
    PositionNotRepresentable {
        vertex_index: usize,
        precision: StaticMeshEncodedPrecision,
    },
    #[error(
        "triangle {triangle_index} corner {corner_index} references vertex {vertex_index}, but the vertex count is {vertex_count}"
    )]
    IndexOutOfRange {
        triangle_index: usize,
        corner_index: usize,
        vertex_index: u32,
        vertex_count: usize,
    },
    #[error("triangle {triangle_index} repeats an index")]
    RepeatedTriangleIndex { triangle_index: usize },
    #[error("triangle {triangle_index} has zero or unrepresentable area")]
    DegenerateTriangle { triangle_index: usize },
    #[error("triangle {triangle_index} collapses in {precision:?}")]
    EncodedDegenerateTriangle {
        triangle_index: usize,
        precision: StaticMeshEncodedPrecision,
    },
    #[error("mesh vertex {vertex_index} is not referenced by a triangle")]
    UnreferencedVertex { vertex_index: usize },
    #[error("mesh export is {actual} bytes; the limit is {maximum} bytes")]
    OutputTooLarge { actual: usize, maximum: usize },
    #[error("{format:?} structure cannot be represented within fixed resource limits")]
    StructureNotRepresentable { format: StaticMeshExportFormat },
    #[error("{format:?} failed its independent post-serialization verification")]
    InternalVerificationFailed { format: StaticMeshExportFormat },
}

/// Admits an untrusted DTO under the hard default limits.
pub fn validate_indexed_triangle_mesh(
    document: &IndexedTriangleMeshV1,
) -> Result<ValidatedIndexedTriangleMesh, StaticMeshExportError> {
    validate_indexed_triangle_mesh_with_limits(document, StaticMeshExportLimits::default())
}

/// Admits an untrusted DTO under caller-tightened limits.
pub fn validate_indexed_triangle_mesh_with_limits(
    document: &IndexedTriangleMeshV1,
    limits: StaticMeshExportLimits,
) -> Result<ValidatedIndexedTriangleMesh, StaticMeshExportError> {
    if document.schema_version != INDEXED_TRIANGLE_MESH_SCHEMA_VERSION_V1 {
        return Err(StaticMeshExportError::UnsupportedSchemaVersion {
            found: document.schema_version,
            latest: INDEXED_TRIANGLE_MESH_SCHEMA_VERSION_V1,
        });
    }
    validate_name(&document.name, limits)?;
    validate_counts(
        document.positions_mm.len(),
        document.triangles.len(),
        limits,
    )?;
    if document.positions_mm.is_empty() {
        return Err(StaticMeshExportError::NoVertices);
    }
    if document.triangles.is_empty() {
        return Err(StaticMeshExportError::NoTriangles);
    }
    if document.normals.len() != document.positions_mm.len() {
        return Err(StaticMeshExportError::NormalCountMismatch {
            actual: document.normals.len(),
            expected: document.positions_mm.len(),
        });
    }

    let mut positions_mm = Vec::new();
    positions_mm
        .try_reserve_exact(document.positions_mm.len())
        .map_err(|_| StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        })?;
    let mut stl_positions = Vec::new();
    stl_positions
        .try_reserve_exact(document.positions_mm.len())
        .map_err(|_| StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::BinaryStl,
        })?;
    let mut glb_positions = Vec::new();
    glb_positions
        .try_reserve_exact(document.positions_mm.len())
        .map_err(|_| StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        })?;
    for (vertex_index, position) in document.positions_mm.iter().copied().enumerate() {
        if position.iter().any(|component| !component.is_finite()) {
            return Err(StaticMeshExportError::NonFinitePosition { vertex_index });
        }
        let canonical = position.map(canonical_zero_f64);
        let stl = encode_position_f32(canonical, 1.0).ok_or(
            StaticMeshExportError::PositionNotRepresentable {
                vertex_index,
                precision: StaticMeshEncodedPrecision::BinaryStlMillimetres,
            },
        )?;
        let glb = encode_position_f32(canonical, 0.001).ok_or(
            StaticMeshExportError::PositionNotRepresentable {
                vertex_index,
                precision: StaticMeshEncodedPrecision::GlbMetres,
            },
        )?;
        positions_mm.push(canonical);
        stl_positions.push(stl);
        glb_positions.push(glb);
    }

    let mut normals = Vec::new();
    normals
        .try_reserve_exact(document.normals.len())
        .map_err(|_| StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        })?;
    for (vertex_index, normal) in document.normals.iter().copied().enumerate() {
        if normal.iter().any(|component| !component.is_finite()) {
            return Err(StaticMeshExportError::NonFiniteNormal { vertex_index });
        }
        let normalized = normalize_vector_f64(normal)
            .ok_or(StaticMeshExportError::InvalidNormal { vertex_index })?;
        let encoded = normalized.map(|component| canonical_zero_f32(component as f32));
        if encoded.iter().any(|component| !component.is_finite())
            || normalize_vector_f64(encoded.map(f64::from)).is_none()
        {
            return Err(StaticMeshExportError::InvalidNormal { vertex_index });
        }
        normals.push(normalized);
    }

    let mut referenced = Vec::new();
    referenced
        .try_reserve_exact(positions_mm.len())
        .map_err(|_| StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        })?;
    referenced.resize(positions_mm.len(), false);
    let mut triangles = Vec::new();
    triangles
        .try_reserve_exact(document.triangles.len())
        .map_err(|_| StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        })?;
    for (triangle_index, triangle) in document.triangles.iter().copied().enumerate() {
        for (corner_index, vertex_index) in triangle.iter().copied().enumerate() {
            let index = usize::try_from(vertex_index).map_err(|_| {
                StaticMeshExportError::IndexOutOfRange {
                    triangle_index,
                    corner_index,
                    vertex_index,
                    vertex_count: positions_mm.len(),
                }
            })?;
            if index >= positions_mm.len() {
                return Err(StaticMeshExportError::IndexOutOfRange {
                    triangle_index,
                    corner_index,
                    vertex_index,
                    vertex_count: positions_mm.len(),
                });
            }
            referenced[index] = true;
        }
        if triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[2] == triangle[0] {
            return Err(StaticMeshExportError::RepeatedTriangleIndex { triangle_index });
        }
        if triangle_normal_f64(indexed_triangle(&positions_mm, triangle)).is_none() {
            return Err(StaticMeshExportError::DegenerateTriangle { triangle_index });
        }
        if triangle_normal_f32(indexed_triangle(&stl_positions, triangle)).is_none() {
            return Err(StaticMeshExportError::EncodedDegenerateTriangle {
                triangle_index,
                precision: StaticMeshEncodedPrecision::BinaryStlMillimetres,
            });
        }
        if triangle_normal_f32(indexed_triangle(&glb_positions, triangle)).is_none() {
            return Err(StaticMeshExportError::EncodedDegenerateTriangle {
                triangle_index,
                precision: StaticMeshEncodedPrecision::GlbMetres,
            });
        }
        triangles.push(triangle);
    }
    if let Some(vertex_index) = referenced.iter().position(|value| !value) {
        return Err(StaticMeshExportError::UnreferencedVertex { vertex_index });
    }

    Ok(ValidatedIndexedTriangleMesh {
        name: document.name.clone(),
        safe_ascii_name: safe_ascii_name(&document.name),
        positions_mm,
        normals,
        triangles,
    })
}

/// Exports one admitted mesh under hard default limits.
pub fn export_static_triangle_mesh(
    format: StaticMeshExportFormat,
    mesh: &ValidatedIndexedTriangleMesh,
) -> Result<StaticMeshExportArtifact, StaticMeshExportError> {
    export_static_triangle_mesh_with_limits(format, mesh, StaticMeshExportLimits::default())
}

/// Exports one admitted mesh under caller-tightened limits.
///
/// Every serializer is followed by a separate bounded structural reader that
/// compares the emitted topology and numeric payload with the admitted mesh.
pub fn export_static_triangle_mesh_with_limits(
    format: StaticMeshExportFormat,
    mesh: &ValidatedIndexedTriangleMesh,
    limits: StaticMeshExportLimits,
) -> Result<StaticMeshExportArtifact, StaticMeshExportError> {
    validate_name(&mesh.name, limits)?;
    validate_counts(mesh.positions_mm.len(), mesh.triangles.len(), limits)?;
    let maximum = limits.max_output_bytes.min(MAX_STATIC_MESH_EXPORT_BYTES);
    let bytes = match format {
        StaticMeshExportFormat::Obj => serialize_obj(mesh, maximum)?,
        StaticMeshExportFormat::BinaryStl => serialize_binary_stl(mesh, maximum)?,
        StaticMeshExportFormat::Glb20 => serialize_glb(mesh, maximum)?,
    };
    if bytes.len() > maximum {
        return Err(StaticMeshExportError::OutputTooLarge {
            actual: bytes.len(),
            maximum,
        });
    }
    let verified = match format {
        StaticMeshExportFormat::Obj => verify_obj(&bytes, mesh, maximum),
        StaticMeshExportFormat::BinaryStl => verify_binary_stl(&bytes, mesh, maximum),
        StaticMeshExportFormat::Glb20 => verify_glb(&bytes, mesh, maximum),
    };
    if !verified {
        return Err(StaticMeshExportError::InternalVerificationFailed { format });
    }
    Ok(StaticMeshExportArtifact {
        format,
        media_type: format.media_type(),
        file_extension: format.file_extension(),
        bytes,
        vertex_count: mesh.positions_mm.len(),
        triangle_count: mesh.triangles.len(),
    })
}

fn validate_name(name: &str, limits: StaticMeshExportLimits) -> Result<(), StaticMeshExportError> {
    let maximum_chars = limits.max_name_chars.min(MAX_STATIC_MESH_NAME_CHARS);
    let actual_chars = name.chars().count();
    if actual_chars > maximum_chars {
        return Err(StaticMeshExportError::NameTooManyCharacters {
            actual: actual_chars,
            maximum: maximum_chars,
        });
    }
    let maximum_bytes = limits.max_name_bytes.min(MAX_STATIC_MESH_NAME_BYTES);
    if name.len() > maximum_bytes {
        return Err(StaticMeshExportError::NameTooManyBytes {
            actual: name.len(),
            maximum: maximum_bytes,
        });
    }
    for (character_index, character) in name.chars().enumerate() {
        if character.is_control() || matches!(character, '\u{2028}' | '\u{2029}') {
            return Err(StaticMeshExportError::InvalidNameCharacter {
                character_index,
                code_point: u32::from(character),
            });
        }
    }
    Ok(())
}

fn validate_counts(
    vertex_count: usize,
    triangle_count: usize,
    limits: StaticMeshExportLimits,
) -> Result<(), StaticMeshExportError> {
    let maximum_vertices = limits.max_vertices.min(MAX_STATIC_MESH_VERTICES);
    if vertex_count > maximum_vertices {
        return Err(StaticMeshExportError::TooManyVertices {
            actual: vertex_count,
            maximum: maximum_vertices,
        });
    }
    let maximum_triangles = limits.max_triangles.min(MAX_STATIC_MESH_TRIANGLES);
    if triangle_count > maximum_triangles {
        return Err(StaticMeshExportError::TooManyTriangles {
            actual: triangle_count,
            maximum: maximum_triangles,
        });
    }
    Ok(())
}

fn safe_ascii_name(name: &str) -> String {
    if name.is_empty() {
        return "Origami2Mesh".to_owned();
    }
    let mut encoded = String::with_capacity(name.len().saturating_mul(3));
    for byte in name.as_bytes().iter().copied() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-') {
            encoded.push(char::from(byte));
        } else {
            write!(&mut encoded, "_{byte:02X}").expect("writing to a String is infallible");
        }
    }
    encoded
}

fn canonical_zero_f64(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn canonical_zero_f32(value: f32) -> f32 {
    if value == 0.0 { 0.0 } else { value }
}

fn encode_position_f32(position: [f64; 3], scale: f64) -> Option<[f32; 3]> {
    let mut encoded = [0.0_f32; 3];
    for (target, component) in encoded.iter_mut().zip(position) {
        let scaled = component * scale;
        if !scaled.is_finite() {
            return None;
        }
        let converted = scaled as f32;
        if !converted.is_finite() {
            return None;
        }
        *target = canonical_zero_f32(converted);
    }
    Some(encoded)
}

fn normalize_vector_f64(vector: [f64; 3]) -> Option<[f64; 3]> {
    let scale = vector
        .iter()
        .map(|component| component.abs())
        .fold(0.0_f64, f64::max);
    if !scale.is_finite() || scale == 0.0 {
        return None;
    }
    let scaled = vector.map(|component| component / scale);
    let length = scaled
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !length.is_finite() || length == 0.0 {
        return None;
    }
    Some(scaled.map(|component| canonical_zero_f64(component / length)))
}

fn triangle_normal_f64(triangle: [[f64; 3]; 3]) -> Option<[f64; 3]> {
    let first = subtract_f64(triangle[1], triangle[0])?;
    let second = subtract_f64(triangle[2], triangle[0])?;
    let scale = first
        .iter()
        .chain(second.iter())
        .map(|component| component.abs())
        .fold(0.0_f64, f64::max);
    if scale == 0.0 || !scale.is_finite() {
        return None;
    }
    let first = first.map(|component| component / scale);
    let second = second.map(|component| component / scale);
    normalize_vector_f64(cross_f64(first, second))
}

fn triangle_normal_f32(triangle: [[f32; 3]; 3]) -> Option<[f32; 3]> {
    let as_f64 = triangle.map(|point| point.map(f64::from));
    triangle_normal_f64(as_f64).map(|normal| normal.map(|value| canonical_zero_f32(value as f32)))
}

fn subtract_f64(left: [f64; 3], right: [f64; 3]) -> Option<[f64; 3]> {
    let difference = [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
    difference
        .iter()
        .all(|component| component.is_finite())
        .then_some(difference)
}

fn cross_f64(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn indexed_triangle<T: Copy>(vertices: &[[T; 3]], triangle: [u32; 3]) -> [[T; 3]; 3] {
    triangle.map(|index| {
        let index = usize::try_from(index).expect("admitted index fits usize");
        vertices[index]
    })
}

fn push_bounded_line(
    output: &mut Vec<u8>,
    line: &str,
    maximum: usize,
) -> Result<(), StaticMeshExportError> {
    if line.len() > MAX_OBJ_LINE_BYTES {
        return Err(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        });
    }
    let actual = output
        .len()
        .checked_add(line.len())
        .and_then(|value| value.checked_add(1))
        .ok_or(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        })?;
    if actual > maximum {
        return Err(StaticMeshExportError::OutputTooLarge { actual, maximum });
    }
    output.try_reserve(line.len() + 1).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        }
    })?;
    output.extend_from_slice(line.as_bytes());
    output.push(b'\n');
    Ok(())
}

fn serialize_obj(
    mesh: &ValidatedIndexedTriangleMesh,
    maximum: usize,
) -> Result<Vec<u8>, StaticMeshExportError> {
    let mut output = Vec::new();
    output.try_reserve(4_096.min(maximum)).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Obj,
        }
    })?;
    push_bounded_line(&mut output, OBJ_HEADER, maximum)?;
    push_bounded_line(&mut output, OBJ_UNIT, maximum)?;
    push_bounded_line(&mut output, OBJ_AXIS, maximum)?;
    push_bounded_line(&mut output, &format!("o {}", mesh.safe_ascii_name), maximum)?;
    for position in &mesh.positions_mm {
        push_bounded_line(
            &mut output,
            &format!(
                "v {} {} {}",
                canonical_zero_f64(position[0]),
                canonical_zero_f64(position[1]),
                canonical_zero_f64(position[2])
            ),
            maximum,
        )?;
    }
    for normal in &mesh.normals {
        push_bounded_line(
            &mut output,
            &format!(
                "vn {} {} {}",
                canonical_zero_f64(normal[0]),
                canonical_zero_f64(normal[1]),
                canonical_zero_f64(normal[2])
            ),
            maximum,
        )?;
    }
    for triangle in &mesh.triangles {
        let indices = triangle.map(|index| u64::from(index) + 1);
        push_bounded_line(
            &mut output,
            &format!(
                "f {}//{} {}//{} {}//{}",
                indices[0], indices[0], indices[1], indices[1], indices[2], indices[2]
            ),
            maximum,
        )?;
    }
    Ok(output)
}

fn verify_obj(bytes: &[u8], mesh: &ValidatedIndexedTriangleMesh, maximum: usize) -> bool {
    if bytes.len() > maximum
        || bytes.is_empty()
        || !bytes.ends_with(b"\n")
        || bytes.contains(&b'\r')
        || bytes.contains(&0)
    {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut lines = text.split_terminator('\n');
    if lines.next() != Some(OBJ_HEADER)
        || lines.next() != Some(OBJ_UNIT)
        || lines.next() != Some(OBJ_AXIS)
        || lines.next() != Some(&format!("o {}", mesh.safe_ascii_name))
    {
        return false;
    }
    for expected in &mesh.positions_mm {
        let Some(line) = lines.next() else {
            return false;
        };
        if line.len() > MAX_OBJ_LINE_BYTES || !verify_obj_vector(line, "v", *expected) {
            return false;
        }
    }
    for expected in &mesh.normals {
        let Some(line) = lines.next() else {
            return false;
        };
        if line.len() > MAX_OBJ_LINE_BYTES || !verify_obj_vector(line, "vn", *expected) {
            return false;
        }
    }
    for expected in &mesh.triangles {
        let Some(line) = lines.next() else {
            return false;
        };
        if line.len() > MAX_OBJ_LINE_BYTES || !verify_obj_face(line, *expected) {
            return false;
        }
    }
    lines.next().is_none()
}

fn verify_obj_vector(line: &str, prefix: &str, expected: [f64; 3]) -> bool {
    let mut fields = line.split(' ');
    if fields.next() != Some(prefix) {
        return false;
    }
    for expected_component in expected {
        let Some(token) = fields.next() else {
            return false;
        };
        let Ok(actual) = token.parse::<f64>() else {
            return false;
        };
        let canonical = canonical_zero_f64(actual);
        if !actual.is_finite()
            || actual.to_bits() != canonical.to_bits()
            || canonical.to_bits() != canonical_zero_f64(expected_component).to_bits()
            || token != canonical.to_string()
        {
            return false;
        }
    }
    fields.next().is_none()
}

fn verify_obj_face(line: &str, expected: [u32; 3]) -> bool {
    let mut fields = line.split(' ');
    if fields.next() != Some("f") {
        return false;
    }
    for expected_index in expected {
        let Some(token) = fields.next() else {
            return false;
        };
        let mut pair = token.split("//");
        let (Some(vertex_token), Some(normal_token), None) =
            (pair.next(), pair.next(), pair.next())
        else {
            return false;
        };
        let expected = u64::from(expected_index) + 1;
        let (Ok(vertex), Ok(normal)) = (vertex_token.parse::<u64>(), normal_token.parse::<u64>())
        else {
            return false;
        };
        if vertex != expected
            || normal != expected
            || vertex.to_string() != vertex_token
            || normal.to_string() != normal_token
        {
            return false;
        }
    }
    fields.next().is_none()
}

fn stl_header(mesh: &ValidatedIndexedTriangleMesh) -> [u8; STL_HEADER_BYTES] {
    let mut header = [b' '; STL_HEADER_BYTES];
    let prefix = b"ORIGAMI2 BINARY STL;UNIT=MM;AXIS=RH_XRIGHT_YFORWARD_ZUP;NAME=";
    let mut cursor = prefix.len();
    header[..cursor].copy_from_slice(prefix);
    let available = STL_HEADER_BYTES - cursor;
    let name = mesh.safe_ascii_name.as_bytes();
    let copied = available.min(name.len());
    header[cursor..cursor + copied].copy_from_slice(&name[..copied]);
    cursor += copied;
    debug_assert!(cursor <= STL_HEADER_BYTES);
    header
}

fn serialize_binary_stl(
    mesh: &ValidatedIndexedTriangleMesh,
    maximum: usize,
) -> Result<Vec<u8>, StaticMeshExportError> {
    let triangle_bytes = mesh.triangles.len().checked_mul(STL_TRIANGLE_BYTES).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::BinaryStl,
        },
    )?;
    let actual = 84_usize.checked_add(triangle_bytes).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::BinaryStl,
        },
    )?;
    if actual > maximum {
        return Err(StaticMeshExportError::OutputTooLarge { actual, maximum });
    }
    let count = u32::try_from(mesh.triangles.len()).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::BinaryStl,
        }
    })?;
    let encoded_positions: Vec<_> = mesh
        .positions_mm
        .iter()
        .copied()
        .map(|position| {
            encode_position_f32(position, 1.0).expect("admission proved STL representability")
        })
        .collect();
    let mut output = Vec::new();
    output.try_reserve_exact(actual).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::BinaryStl,
        }
    })?;
    output.extend_from_slice(&stl_header(mesh));
    output.extend_from_slice(&count.to_le_bytes());
    for triangle in &mesh.triangles {
        let vertices = indexed_triangle(&encoded_positions, *triangle);
        let normal = triangle_normal_f32(vertices).expect("admission proved STL triangle area");
        for component in normal {
            output.extend_from_slice(&component.to_le_bytes());
        }
        for vertex in vertices {
            for component in vertex {
                output.extend_from_slice(&component.to_le_bytes());
            }
        }
        output.extend_from_slice(&0_u16.to_le_bytes());
    }
    debug_assert_eq!(output.len(), actual);
    Ok(output)
}

fn verify_binary_stl(bytes: &[u8], mesh: &ValidatedIndexedTriangleMesh, maximum: usize) -> bool {
    if bytes.len() > maximum || bytes.len() < 84 || bytes[..80] != stl_header(mesh) {
        return false;
    }
    let Some(count_bytes) = bytes.get(80..84).and_then(|value| value.try_into().ok()) else {
        return false;
    };
    let count = u32::from_le_bytes(count_bytes);
    if usize::try_from(count).ok() != Some(mesh.triangles.len()) {
        return false;
    }
    let Some(expected_length) = usize::try_from(count)
        .ok()
        .and_then(|count| count.checked_mul(STL_TRIANGLE_BYTES))
        .and_then(|records| records.checked_add(84))
    else {
        return false;
    };
    if bytes.len() != expected_length {
        return false;
    }
    let encoded_positions: Vec<_> = mesh
        .positions_mm
        .iter()
        .copied()
        .map(|position| encode_position_f32(position, 1.0))
        .collect::<Option<_>>()
        .unwrap_or_default();
    if encoded_positions.len() != mesh.positions_mm.len() {
        return false;
    }
    let mut cursor = 84;
    for triangle in &mesh.triangles {
        let vertices = indexed_triangle(&encoded_positions, *triangle);
        let Some(normal) = triangle_normal_f32(vertices) else {
            return false;
        };
        for expected in normal.into_iter().chain(vertices.into_iter().flatten()) {
            let Some(actual) = read_f32_le(bytes, &mut cursor) else {
                return false;
            };
            if !actual.is_finite()
                || actual.to_bits() != canonical_zero_f32(actual).to_bits()
                || actual.to_bits() != expected.to_bits()
            {
                return false;
            }
        }
        let Some(attribute_bytes) = bytes
            .get(cursor..cursor + 2)
            .and_then(|value| value.try_into().ok())
        else {
            return false;
        };
        if u16::from_le_bytes(attribute_bytes) != 0 {
            return false;
        }
        cursor += 2;
    }
    cursor == bytes.len()
}

#[derive(Serialize)]
struct GlbRoot<'a> {
    asset: GlbAsset,
    scene: u32,
    scenes: [GlbScene<'a>; 1],
    nodes: [GlbNode<'a>; 1],
    meshes: [GlbMesh<'a>; 1],
    buffers: [GlbBuffer; 1],
    #[serde(rename = "bufferViews")]
    buffer_views: [GlbBufferView; 3],
    accessors: [GlbAccessor; 3],
}

#[derive(Serialize)]
struct GlbAsset {
    version: &'static str,
    generator: &'static str,
    extras: GlbAssetExtras,
}

#[derive(Serialize)]
struct GlbAssetExtras {
    #[serde(rename = "origami2Unit")]
    unit: &'static str,
    #[serde(rename = "origami2SourceUnit")]
    source_unit: &'static str,
    #[serde(rename = "origami2SourceAxis")]
    source_axis: &'static str,
    #[serde(rename = "origami2NodeAxisConversion")]
    node_axis_conversion: &'static str,
}

#[derive(Serialize)]
struct GlbScene<'a> {
    name: &'a str,
    nodes: [u32; 1],
}

#[derive(Serialize)]
struct GlbNode<'a> {
    name: &'a str,
    mesh: u32,
    matrix: [f32; 16],
}

#[derive(Serialize)]
struct GlbMesh<'a> {
    name: &'a str,
    primitives: [GlbPrimitive; 1],
}

#[derive(Serialize)]
struct GlbPrimitive {
    attributes: GlbAttributes,
    indices: u32,
    mode: u32,
}

#[derive(Serialize)]
struct GlbAttributes {
    #[serde(rename = "POSITION")]
    position: u32,
    #[serde(rename = "NORMAL")]
    normal: u32,
}

#[derive(Serialize)]
struct GlbBuffer {
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

#[derive(Serialize)]
struct GlbBufferView {
    buffer: u32,
    #[serde(rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    target: u32,
}

#[derive(Serialize)]
struct GlbAccessor {
    #[serde(rename = "bufferView")]
    buffer_view: u32,
    #[serde(rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    accessor_type: &'static str,
    min: GlbAccessorBounds,
    max: GlbAccessorBounds,
}

#[derive(Serialize)]
#[serde(untagged)]
enum GlbAccessorBounds {
    Vec3([f32; 3]),
    Scalar([u32; 1]),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbRoot {
    asset: CheckedGlbAsset,
    scene: u32,
    scenes: Vec<CheckedGlbScene>,
    nodes: Vec<CheckedGlbNode>,
    meshes: Vec<CheckedGlbMesh>,
    buffers: Vec<CheckedGlbBuffer>,
    #[serde(rename = "bufferViews")]
    buffer_views: Vec<CheckedGlbBufferView>,
    accessors: Vec<CheckedGlbAccessor>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbAsset {
    version: String,
    generator: String,
    extras: CheckedGlbAssetExtras,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbAssetExtras {
    #[serde(rename = "origami2Unit")]
    unit: String,
    #[serde(rename = "origami2SourceUnit")]
    source_unit: String,
    #[serde(rename = "origami2SourceAxis")]
    source_axis: String,
    #[serde(rename = "origami2NodeAxisConversion")]
    node_axis_conversion: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbScene {
    name: String,
    nodes: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbNode {
    name: String,
    mesh: u32,
    matrix: Vec<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbMesh {
    name: String,
    primitives: Vec<CheckedGlbPrimitive>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbPrimitive {
    attributes: CheckedGlbAttributes,
    indices: u32,
    mode: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbAttributes {
    #[serde(rename = "POSITION")]
    position: u32,
    #[serde(rename = "NORMAL")]
    normal: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbBuffer {
    #[serde(rename = "byteLength")]
    byte_length: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbBufferView {
    buffer: u32,
    #[serde(rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    target: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbAccessor {
    #[serde(rename = "bufferView")]
    buffer_view: u32,
    #[serde(rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    accessor_type: String,
    min: Vec<serde_json::Number>,
    max: Vec<serde_json::Number>,
}

fn serialize_glb(
    mesh: &ValidatedIndexedTriangleMesh,
    maximum: usize,
) -> Result<Vec<u8>, StaticMeshExportError> {
    let position_bytes = mesh.positions_mm.len().checked_mul(12).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let normal_bytes = position_bytes;
    let index_count = mesh.triangles.len().checked_mul(3).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let index_bytes =
        index_count
            .checked_mul(4)
            .ok_or(StaticMeshExportError::StructureNotRepresentable {
                format: StaticMeshExportFormat::Glb20,
            })?;
    let normal_offset = position_bytes;
    let index_offset = normal_offset.checked_add(normal_bytes).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let binary_length = index_offset.checked_add(index_bytes).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    if normal_offset % 4 != 0 || index_offset % 4 != 0 || binary_length % 4 != 0 {
        return Err(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        });
    }

    let positions: Vec<_> = mesh
        .positions_mm
        .iter()
        .copied()
        .map(|position| {
            encode_position_f32(position, 0.001).expect("admission proved GLB representability")
        })
        .collect();
    let normals: Vec<_> = mesh
        .normals
        .iter()
        .map(|normal| normal.map(|component| canonical_zero_f32(component as f32)))
        .collect();
    let (position_min, position_max) =
        vec3_bounds(&positions).ok_or(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        })?;
    let (normal_min, normal_max) =
        vec3_bounds(&normals).ok_or(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        })?;
    let (index_min, index_max) =
        index_bounds(&mesh.triangles).ok_or(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        })?;
    let root = GlbRoot {
        asset: GlbAsset {
            version: "2.0",
            generator: "ORIGAMI2",
            extras: GlbAssetExtras {
                unit: "meter",
                source_unit: STATIC_MESH_SOURCE_UNIT,
                source_axis: STATIC_MESH_SOURCE_AXIS,
                node_axis_conversion: "local (x,y,z) -> glTF scene (-x,z,y)",
            },
        },
        scene: 0,
        scenes: [GlbScene {
            name: &mesh.name,
            nodes: [0],
        }],
        nodes: [GlbNode {
            name: &mesh.name,
            mesh: 0,
            matrix: GLTF_NODE_MATRIX,
        }],
        meshes: [GlbMesh {
            name: &mesh.name,
            primitives: [GlbPrimitive {
                attributes: GlbAttributes {
                    position: 0,
                    normal: 1,
                },
                indices: 2,
                mode: GLTF_TRIANGLES,
            }],
        }],
        buffers: [GlbBuffer {
            byte_length: binary_length,
        }],
        buffer_views: [
            GlbBufferView {
                buffer: 0,
                byte_offset: 0,
                byte_length: position_bytes,
                target: GLTF_ARRAY_BUFFER,
            },
            GlbBufferView {
                buffer: 0,
                byte_offset: normal_offset,
                byte_length: normal_bytes,
                target: GLTF_ARRAY_BUFFER,
            },
            GlbBufferView {
                buffer: 0,
                byte_offset: index_offset,
                byte_length: index_bytes,
                target: GLTF_ELEMENT_ARRAY_BUFFER,
            },
        ],
        accessors: [
            GlbAccessor {
                buffer_view: 0,
                byte_offset: 0,
                component_type: GLTF_FLOAT,
                count: positions.len(),
                accessor_type: "VEC3",
                min: GlbAccessorBounds::Vec3(position_min),
                max: GlbAccessorBounds::Vec3(position_max),
            },
            GlbAccessor {
                buffer_view: 1,
                byte_offset: 0,
                component_type: GLTF_FLOAT,
                count: normals.len(),
                accessor_type: "VEC3",
                min: GlbAccessorBounds::Vec3(normal_min),
                max: GlbAccessorBounds::Vec3(normal_max),
            },
            GlbAccessor {
                buffer_view: 2,
                byte_offset: 0,
                component_type: GLTF_UNSIGNED_INT,
                count: index_count,
                accessor_type: "SCALAR",
                min: GlbAccessorBounds::Scalar([index_min]),
                max: GlbAccessorBounds::Scalar([index_max]),
            },
        ],
    };
    let json = serde_json::to_vec(&root).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        }
    })?;
    if json.len() > MAX_GLB_JSON_BYTES {
        return Err(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        });
    }
    let json_padded_length =
        align4(json.len()).ok_or(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        })?;
    let total_length = GLB_HEADER_BYTES
        .checked_add(GLB_CHUNK_HEADER_BYTES)
        .and_then(|value| value.checked_add(json_padded_length))
        .and_then(|value| value.checked_add(GLB_CHUNK_HEADER_BYTES))
        .and_then(|value| value.checked_add(binary_length))
        .ok_or(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        })?;
    if total_length > maximum {
        return Err(StaticMeshExportError::OutputTooLarge {
            actual: total_length,
            maximum,
        });
    }
    let total_length_u32 = u32::try_from(total_length).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        }
    })?;
    let json_length_u32 = u32::try_from(json_padded_length).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        }
    })?;
    let binary_length_u32 = u32::try_from(binary_length).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        }
    })?;

    let mut output = Vec::new();
    output.try_reserve_exact(total_length).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        }
    })?;
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2_u32.to_le_bytes());
    output.extend_from_slice(&total_length_u32.to_le_bytes());
    output.extend_from_slice(&json_length_u32.to_le_bytes());
    output.extend_from_slice(&GLB_JSON_CHUNK_TYPE.to_le_bytes());
    output.extend_from_slice(&json);
    output.resize(output.len() + (json_padded_length - json.len()), b' ');
    output.extend_from_slice(&binary_length_u32.to_le_bytes());
    output.extend_from_slice(&GLB_BIN_CHUNK_TYPE.to_le_bytes());
    for position in positions {
        for component in position {
            output.extend_from_slice(&component.to_le_bytes());
        }
    }
    for normal in normals {
        for component in normal {
            output.extend_from_slice(&component.to_le_bytes());
        }
    }
    for triangle in &mesh.triangles {
        for index in triangle {
            output.extend_from_slice(&index.to_le_bytes());
        }
    }
    debug_assert_eq!(output.len(), total_length);
    Ok(output)
}

fn verify_glb(bytes: &[u8], mesh: &ValidatedIndexedTriangleMesh, maximum: usize) -> bool {
    if bytes.len() > maximum || bytes.len() < 28 || bytes.get(..4) != Some(b"glTF") {
        return false;
    }
    let Some(version) = read_u32_le_at(bytes, 4) else {
        return false;
    };
    let Some(declared_length) = read_u32_le_at(bytes, 8) else {
        return false;
    };
    if version != 2 || usize::try_from(declared_length).ok() != Some(bytes.len()) {
        return false;
    }
    let Some(json_length) = read_u32_le_at(bytes, 12).and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if json_length == 0
        || json_length > MAX_GLB_JSON_BYTES
        || json_length % 4 != 0
        || read_u32_le_at(bytes, 16) != Some(GLB_JSON_CHUNK_TYPE)
    {
        return false;
    }
    let json_start = 20_usize;
    let Some(json_end) = json_start.checked_add(json_length) else {
        return false;
    };
    let Some(json_chunk) = bytes.get(json_start..json_end) else {
        return false;
    };
    let Some(last_json_byte) = json_chunk.iter().rposition(|byte| *byte != b' ') else {
        return false;
    };
    let padding_length = json_chunk.len() - last_json_byte - 1;
    if padding_length > 3
        || json_chunk[last_json_byte + 1..]
            .iter()
            .any(|byte| *byte != b' ')
    {
        return false;
    }
    let json = &json_chunk[..=last_json_byte];
    let Ok(root) = serde_json::from_slice::<CheckedGlbRoot>(json) else {
        return false;
    };
    let Some(binary_header_end) = json_end.checked_add(GLB_CHUNK_HEADER_BYTES) else {
        return false;
    };
    let Some(binary_header) = bytes.get(json_end..binary_header_end) else {
        return false;
    };
    let Some(binary_length) =
        read_u32_le_at(binary_header, 0).and_then(|value| usize::try_from(value).ok())
    else {
        return false;
    };
    if read_u32_le_at(binary_header, 4) != Some(GLB_BIN_CHUNK_TYPE) || binary_length % 4 != 0 {
        return false;
    }
    let Some(binary_end) = binary_header_end.checked_add(binary_length) else {
        return false;
    };
    let Some(binary) = bytes.get(binary_header_end..binary_end) else {
        return false;
    };
    if binary_end != bytes.len() || !verify_glb_structure(&root, mesh, binary_length) {
        return false;
    }
    verify_glb_binary(&root, binary, mesh)
}

fn verify_glb_structure(
    root: &CheckedGlbRoot,
    mesh: &ValidatedIndexedTriangleMesh,
    binary_length: usize,
) -> bool {
    if root.asset.version != "2.0"
        || root.asset.generator != "ORIGAMI2"
        || root.asset.extras.unit != "meter"
        || root.asset.extras.source_unit != STATIC_MESH_SOURCE_UNIT
        || root.asset.extras.source_axis != STATIC_MESH_SOURCE_AXIS
        || root.asset.extras.node_axis_conversion != "local (x,y,z) -> glTF scene (-x,z,y)"
        || root.scene != 0
        || root.scenes.len() != 1
        || root.scenes[0].name != mesh.name
        || root.scenes[0].nodes != [0]
        || root.nodes.len() != 1
        || root.nodes[0].name != mesh.name
        || root.nodes[0].mesh != 0
        || root.nodes[0].matrix.len() != GLTF_NODE_MATRIX.len()
        || !root.nodes[0]
            .matrix
            .iter()
            .zip(GLTF_NODE_MATRIX)
            .all(|(actual, expected)| actual.to_bits() == expected.to_bits())
        || root.meshes.len() != 1
        || root.meshes[0].name != mesh.name
        || root.meshes[0].primitives.len() != 1
        || root.meshes[0].primitives[0].attributes.position != 0
        || root.meshes[0].primitives[0].attributes.normal != 1
        || root.meshes[0].primitives[0].indices != 2
        || root.meshes[0].primitives[0].mode != GLTF_TRIANGLES
        || root.buffers.len() != 1
        || root.buffers[0].byte_length != binary_length
        || root.buffer_views.len() != 3
        || root.accessors.len() != 3
    {
        return false;
    }
    let position_bytes = match mesh.positions_mm.len().checked_mul(12) {
        Some(value) => value,
        None => return false,
    };
    let normal_offset = position_bytes;
    let index_offset = match normal_offset.checked_add(position_bytes) {
        Some(value) => value,
        None => return false,
    };
    let index_count = match mesh.triangles.len().checked_mul(3) {
        Some(value) => value,
        None => return false,
    };
    let index_bytes = match index_count.checked_mul(4) {
        Some(value) => value,
        None => return false,
    };
    let expected_views = [
        (0, 0, position_bytes, GLTF_ARRAY_BUFFER),
        (0, normal_offset, position_bytes, GLTF_ARRAY_BUFFER),
        (0, index_offset, index_bytes, GLTF_ELEMENT_ARRAY_BUFFER),
    ];
    if !root
        .buffer_views
        .iter()
        .zip(expected_views)
        .all(|(actual, expected)| {
            actual.buffer == expected.0
                && actual.byte_offset == expected.1
                && actual.byte_length == expected.2
                && actual.target == expected.3
                && actual.byte_offset % 4 == 0
        })
    {
        return false;
    }
    let expected_accessors = [
        (0, GLTF_FLOAT, mesh.positions_mm.len(), "VEC3"),
        (1, GLTF_FLOAT, mesh.normals.len(), "VEC3"),
        (2, GLTF_UNSIGNED_INT, index_count, "SCALAR"),
    ];
    root.accessors
        .iter()
        .zip(expected_accessors)
        .all(|(actual, expected)| {
            actual.buffer_view == expected.0
                && actual.byte_offset == 0
                && actual.component_type == expected.1
                && actual.count == expected.2
                && actual.accessor_type == expected.3
        })
}

fn verify_glb_binary(
    root: &CheckedGlbRoot,
    binary: &[u8],
    mesh: &ValidatedIndexedTriangleMesh,
) -> bool {
    let positions: Vec<_> = mesh
        .positions_mm
        .iter()
        .copied()
        .map(|position| encode_position_f32(position, 0.001))
        .collect::<Option<_>>()
        .unwrap_or_default();
    let normals: Vec<_> = mesh
        .normals
        .iter()
        .map(|normal| normal.map(|component| canonical_zero_f32(component as f32)))
        .collect();
    if positions.len() != mesh.positions_mm.len() {
        return false;
    }
    let mut cursor = 0;
    for expected in positions.iter().chain(normals.iter()).flatten() {
        let Some(actual) = read_f32_le(binary, &mut cursor) else {
            return false;
        };
        if !actual.is_finite()
            || canonical_zero_f32(actual).to_bits() != actual.to_bits()
            || actual.to_bits() != expected.to_bits()
        {
            return false;
        }
    }
    for expected in mesh.triangles.iter().flatten() {
        let Some(actual) = read_u32_le(binary, &mut cursor) else {
            return false;
        };
        if actual != *expected
            || usize::try_from(actual)
                .map(|value| value >= mesh.positions_mm.len())
                .unwrap_or(true)
        {
            return false;
        }
    }
    if cursor != binary.len() {
        return false;
    }
    let Some((position_min, position_max)) = vec3_bounds(&positions) else {
        return false;
    };
    let Some((normal_min, normal_max)) = vec3_bounds(&normals) else {
        return false;
    };
    let Some((index_min, index_max)) = index_bounds(&mesh.triangles) else {
        return false;
    };
    check_number_vec_f32(&root.accessors[0].min, position_min)
        && check_number_vec_f32(&root.accessors[0].max, position_max)
        && check_number_vec_f32(&root.accessors[1].min, normal_min)
        && check_number_vec_f32(&root.accessors[1].max, normal_max)
        && check_number_vec_u32(&root.accessors[2].min, [index_min])
        && check_number_vec_u32(&root.accessors[2].max, [index_max])
}

fn check_number_vec_f32<const N: usize>(actual: &[serde_json::Number], expected: [f32; N]) -> bool {
    actual.len() == N
        && actual.iter().zip(expected).all(|(actual, expected)| {
            let text = actual.to_string();
            text.parse::<f32>()
                .map(|value| {
                    value.is_finite()
                        && canonical_zero_f32(value).to_bits() == expected.to_bits()
                        && serde_json::to_string(&expected).ok().as_deref() == Some(text.as_str())
                })
                .unwrap_or(false)
        })
}

fn check_number_vec_u32<const N: usize>(actual: &[serde_json::Number], expected: [u32; N]) -> bool {
    actual.len() == N
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.as_u64() == Some(u64::from(expected)))
}

fn vec3_bounds(values: &[[f32; 3]]) -> Option<([f32; 3], [f32; 3])> {
    let first = *values.first()?;
    let mut minimum = first;
    let mut maximum = first;
    for value in values.iter().copied().skip(1) {
        for component in 0..3 {
            minimum[component] = canonical_zero_f32(minimum[component].min(value[component]));
            maximum[component] = canonical_zero_f32(maximum[component].max(value[component]));
        }
    }
    Some((minimum, maximum))
}

fn index_bounds(triangles: &[[u32; 3]]) -> Option<(u32, u32)> {
    let first = *triangles.first()?.first()?;
    let mut minimum = first;
    let mut maximum = first;
    for index in triangles.iter().flatten().copied() {
        minimum = minimum.min(index);
        maximum = maximum.max(index);
    }
    Some((minimum, maximum))
}

fn align4(value: usize) -> Option<usize> {
    value.checked_add(3).map(|value| value & !3)
}

fn read_f32_le(bytes: &[u8], cursor: &mut usize) -> Option<f32> {
    let end = cursor.checked_add(4)?;
    let array = bytes.get(*cursor..end)?.try_into().ok()?;
    *cursor = end;
    Some(f32::from_le_bytes(array))
}

fn read_u32_le(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    let end = cursor.checked_add(4)?;
    let array = bytes.get(*cursor..end)?.try_into().ok()?;
    *cursor = end;
    Some(u32::from_le_bytes(array))
}

fn read_u32_le_at(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let array = bytes.get(offset..end)?.try_into().ok()?;
    Some(u32::from_le_bytes(array))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_document() -> IndexedTriangleMeshV1 {
        IndexedTriangleMeshV1::new(
            "折り紙 #1",
            vec![
                [-0.0, 0.0, 0.0],
                [100.0, -0.0, 0.0],
                [100.0, 50.0, 0.0],
                [0.0, 50.0, 0.0],
            ],
            vec![[0.0, 0.0, 2.0]; 4],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    fn sample_mesh() -> ValidatedIndexedTriangleMesh {
        validate_indexed_triangle_mesh(&sample_document()).expect("sample mesh")
    }

    fn limits() -> StaticMeshExportLimits {
        StaticMeshExportLimits::default()
    }

    #[test]
    fn one_admitted_mesh_exports_three_verified_deterministic_formats() {
        let mesh = sample_mesh();
        for format in [
            StaticMeshExportFormat::Obj,
            StaticMeshExportFormat::BinaryStl,
            StaticMeshExportFormat::Glb20,
        ] {
            let first = export_static_triangle_mesh(format, &mesh).expect("first export");
            let second = export_static_triangle_mesh(format, &mesh).expect("second export");
            assert_eq!(first, second);
            assert_eq!(first.vertex_count, 4);
            assert_eq!(first.triangle_count, 2);
            assert_eq!(first.media_type, format.media_type());
            assert_eq!(first.file_extension, format.file_extension());
            assert!(!first.bytes.is_empty());
        }
    }

    #[test]
    fn schema_version_is_exact_and_unknown_fields_are_rejected_by_serde() {
        let mut document = sample_document();
        document.schema_version += 1;
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::UnsupportedSchemaVersion {
                found: 2,
                latest: 1
            })
        );
        let json = serde_json::json!({
            "schema_version": 1,
            "name": "mesh",
            "positions_mm": [[0,0,0],[1,0,0],[0,1,0]],
            "normals": [[0,0,1],[0,0,1],[0,0,1]],
            "triangles": [[0,1,2]],
            "future": true
        });
        assert!(serde_json::from_value::<IndexedTriangleMeshV1>(json).is_err());
    }

    #[test]
    fn finite_values_are_required_and_negative_zero_is_canonicalized_everywhere() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut document = sample_document();
            document.positions_mm[0][0] = value;
            assert_eq!(
                validate_indexed_triangle_mesh(&document),
                Err(StaticMeshExportError::NonFinitePosition { vertex_index: 0 })
            );
            let mut document = sample_document();
            document.normals[1][2] = value;
            assert_eq!(
                validate_indexed_triangle_mesh(&document),
                Err(StaticMeshExportError::NonFiniteNormal { vertex_index: 1 })
            );
        }

        let mesh = sample_mesh();
        for vector in mesh.positions_mm.iter().chain(mesh.normals.iter()) {
            for value in vector {
                assert_ne!(value.to_bits(), (-0.0_f64).to_bits());
            }
        }
        let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
            .expect("OBJ")
            .bytes;
        assert!(!String::from_utf8(obj).expect("OBJ UTF-8").contains("-0"));
        let stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh)
            .expect("STL")
            .bytes;
        assert!(verify_binary_stl(&stl, &mesh, stl.len()));
        let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
            .expect("GLB")
            .bytes;
        assert!(verify_glb(&glb, &mesh, glb.len()));
    }

    #[test]
    fn normals_are_counted_validated_and_canonically_normalized() {
        let mut document = sample_document();
        document.normals.pop();
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::NormalCountMismatch {
                actual: 3,
                expected: 4
            })
        );
        let mut document = sample_document();
        document.normals[0] = [0.0, 0.0, 0.0];
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::InvalidNormal { vertex_index: 0 })
        );
        let mesh = sample_mesh();
        assert_eq!(mesh.normals[0], [0.0, 0.0, 1.0]);
    }

    #[test]
    fn empty_and_unreferenced_mesh_content_is_rejected() {
        let empty = IndexedTriangleMeshV1::new("", vec![], vec![], vec![]);
        assert_eq!(
            validate_indexed_triangle_mesh(&empty),
            Err(StaticMeshExportError::NoVertices)
        );
        let mut no_triangles = sample_document();
        no_triangles.triangles.clear();
        assert_eq!(
            validate_indexed_triangle_mesh(&no_triangles),
            Err(StaticMeshExportError::NoTriangles)
        );
        let mut unreferenced = sample_document();
        unreferenced.triangles.pop();
        assert_eq!(
            validate_indexed_triangle_mesh(&unreferenced),
            Err(StaticMeshExportError::UnreferencedVertex { vertex_index: 3 })
        );
    }

    #[test]
    fn indices_and_geometric_degeneracy_fail_closed() {
        let mut out_of_range = sample_document();
        out_of_range.triangles[0][2] = 4;
        assert_eq!(
            validate_indexed_triangle_mesh(&out_of_range),
            Err(StaticMeshExportError::IndexOutOfRange {
                triangle_index: 0,
                corner_index: 2,
                vertex_index: 4,
                vertex_count: 4
            })
        );
        let mut repeated = sample_document();
        repeated.triangles[0] = [0, 1, 1];
        assert_eq!(
            validate_indexed_triangle_mesh(&repeated),
            Err(StaticMeshExportError::RepeatedTriangleIndex { triangle_index: 0 })
        );
        let collinear = IndexedTriangleMeshV1::new(
            "line",
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            vec![[0.0, 0.0, 1.0]; 3],
            vec![[0, 1, 2]],
        );
        assert_eq!(
            validate_indexed_triangle_mesh(&collinear),
            Err(StaticMeshExportError::DegenerateTriangle { triangle_index: 0 })
        );
    }

    #[test]
    fn f32_overflow_and_precision_collapse_are_rejected_before_export() {
        let huge = IndexedTriangleMeshV1::new(
            "huge",
            vec![[f64::MAX, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            vec![[0.0, 0.0, 1.0]; 3],
            vec![[0, 1, 2]],
        );
        assert_eq!(
            validate_indexed_triangle_mesh(&huge),
            Err(StaticMeshExportError::PositionNotRepresentable {
                vertex_index: 0,
                precision: StaticMeshEncodedPrecision::BinaryStlMillimetres
            })
        );

        let next = f64::from_bits(1.0_f64.to_bits() + 1);
        let collapsed = IndexedTriangleMeshV1::new(
            "collapse",
            vec![[1.0, 0.0, 0.0], [next, 0.0, 0.0], [1.0, 1.0, 0.0]],
            vec![[0.0, 0.0, 1.0]; 3],
            vec![[0, 1, 2]],
        );
        assert_eq!(
            validate_indexed_triangle_mesh(&collapsed),
            Err(StaticMeshExportError::EncodedDegenerateTriangle {
                triangle_index: 0,
                precision: StaticMeshEncodedPrecision::BinaryStlMillimetres
            })
        );
    }

    #[test]
    fn vertex_triangle_and_name_limits_accept_exact_and_reject_one_short() {
        let document = sample_document();
        let mut exact = limits();
        exact.max_vertices = 4;
        exact.max_triangles = 2;
        exact.max_name_chars = document.name.chars().count();
        exact.max_name_bytes = document.name.len();
        validate_indexed_triangle_mesh_with_limits(&document, exact).expect("exact limits");

        let mut one_short = exact;
        one_short.max_vertices = 3;
        assert_eq!(
            validate_indexed_triangle_mesh_with_limits(&document, one_short),
            Err(StaticMeshExportError::TooManyVertices {
                actual: 4,
                maximum: 3
            })
        );
        one_short = exact;
        one_short.max_triangles = 1;
        assert_eq!(
            validate_indexed_triangle_mesh_with_limits(&document, one_short),
            Err(StaticMeshExportError::TooManyTriangles {
                actual: 2,
                maximum: 1
            })
        );
        one_short = exact;
        one_short.max_name_chars -= 1;
        assert!(matches!(
            validate_indexed_triangle_mesh_with_limits(&document, one_short),
            Err(StaticMeshExportError::NameTooManyCharacters { .. })
        ));
        one_short = exact;
        one_short.max_name_bytes -= 1;
        assert!(matches!(
            validate_indexed_triangle_mesh_with_limits(&document, one_short),
            Err(StaticMeshExportError::NameTooManyBytes { .. })
        ));
    }

    #[test]
    fn hard_name_limits_cannot_be_relaxed_by_a_caller() {
        let mut document = sample_document();
        document.name = "a".repeat(MAX_STATIC_MESH_NAME_CHARS + 1);
        let relaxed = StaticMeshExportLimits {
            max_name_chars: usize::MAX,
            max_name_bytes: usize::MAX,
            ..limits()
        };
        assert_eq!(
            validate_indexed_triangle_mesh_with_limits(&document, relaxed),
            Err(StaticMeshExportError::NameTooManyCharacters {
                actual: MAX_STATIC_MESH_NAME_CHARS + 1,
                maximum: MAX_STATIC_MESH_NAME_CHARS
            })
        );
    }

    #[test]
    fn output_byte_limits_accept_exact_and_reject_one_short_for_each_format() {
        let mesh = sample_mesh();
        for format in [
            StaticMeshExportFormat::Obj,
            StaticMeshExportFormat::BinaryStl,
            StaticMeshExportFormat::Glb20,
        ] {
            let artifact = export_static_triangle_mesh(format, &mesh).expect("baseline");
            let exact = StaticMeshExportLimits {
                max_output_bytes: artifact.bytes.len(),
                ..limits()
            };
            assert_eq!(
                export_static_triangle_mesh_with_limits(format, &mesh, exact)
                    .expect("exact byte limit")
                    .bytes,
                artifact.bytes
            );
            let one_short = StaticMeshExportLimits {
                max_output_bytes: artifact.bytes.len() - 1,
                ..limits()
            };
            assert!(matches!(
                export_static_triangle_mesh_with_limits(format, &mesh, one_short),
                Err(StaticMeshExportError::OutputTooLarge { maximum, .. })
                    if maximum == artifact.bytes.len() - 1
            ));
        }
    }

    #[test]
    fn malicious_name_is_encoded_or_json_escaped_without_record_injection() {
        let mut document = sample_document();
        document.name = "x #\u{304a}\u{308a}\"} , \"nodes\":[{\"mesh\":99}]".to_owned();
        let mesh = validate_indexed_triangle_mesh(&document).expect("safe escaped name");
        let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
            .expect("OBJ")
            .bytes;
        let obj = String::from_utf8(obj).expect("OBJ UTF-8");
        assert_eq!(obj.lines().filter(|line| line.starts_with("o ")).count(), 1);
        assert!(!obj.lines().any(|line| line.starts_with("x #")));
        assert!(obj.contains("_23"));

        let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
            .expect("GLB")
            .bytes;
        assert!(verify_glb(&glb, &mesh, glb.len()));

        document.name = "bad\nobject".to_owned();
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::InvalidNameCharacter {
                character_index: 3,
                code_point: 0x0a
            })
        );
        document.name = "bad\u{2028}object".to_owned();
        assert!(matches!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::InvalidNameCharacter {
                code_point: 0x2028,
                ..
            })
        ));
    }

    #[test]
    fn obj_checker_rejects_noncanonical_number_and_changed_face() {
        let mesh = sample_mesh();
        let artifact =
            export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh).expect("OBJ");
        let mut text = String::from_utf8(artifact.bytes).expect("UTF-8");
        text = text.replacen("v 0 0 0", "v -0 0 0", 1);
        assert!(!verify_obj(text.as_bytes(), &mesh, text.len()));

        let artifact =
            export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh).expect("OBJ");
        let mut text = String::from_utf8(artifact.bytes).expect("UTF-8");
        text = text.replacen("f 1//1 2//2 3//3", "f 1//1 3//3 2//2", 1);
        assert!(!verify_obj(text.as_bytes(), &mesh, text.len()));

        let artifact =
            export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh).expect("OBJ");
        let mut text = String::from_utf8(artifact.bytes).expect("UTF-8");
        text = text.replacen("f 1//1 2//2 3//3", "f 1//01 2//2 3//3", 1);
        assert!(!verify_obj(text.as_bytes(), &mesh, text.len()));
    }

    #[test]
    fn binary_stl_header_count_size_endianness_and_attributes_are_strict() {
        let mesh = sample_mesh();
        let artifact =
            export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh).expect("STL");
        assert_eq!(artifact.bytes.len(), 84 + 2 * STL_TRIANGLE_BYTES);
        assert_eq!(&artifact.bytes[..80], &stl_header(&mesh));
        assert_eq!(&artifact.bytes[80..84], &2_u32.to_le_bytes());

        let mut big_endian_count = artifact.bytes.clone();
        big_endian_count[80..84].copy_from_slice(&2_u32.to_be_bytes());
        assert!(!verify_binary_stl(
            &big_endian_count,
            &mesh,
            big_endian_count.len()
        ));
        let mut bad_attribute = artifact.bytes.clone();
        bad_attribute[132] = 1;
        assert!(!verify_binary_stl(
            &bad_attribute,
            &mesh,
            bad_attribute.len()
        ));
        let mut truncated = artifact.bytes;
        truncated.pop();
        assert!(!verify_binary_stl(&truncated, &mesh, truncated.len()));
    }

    #[test]
    fn glb_has_aligned_chunks_fixed_property_order_and_strict_structure() {
        let mesh = sample_mesh();
        let artifact =
            export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).expect("GLB");
        let bytes = artifact.bytes;
        assert_eq!(&bytes[..4], b"glTF");
        assert_eq!(read_u32_le_at(&bytes, 4), Some(2));
        assert_eq!(
            read_u32_le_at(&bytes, 8).and_then(|value| usize::try_from(value).ok()),
            Some(bytes.len())
        );
        let json_length = usize::try_from(read_u32_le_at(&bytes, 12).expect("JSON length"))
            .expect("usize JSON length");
        assert_eq!(json_length % 4, 0);
        let json_chunk = &bytes[20..20 + json_length];
        let json = std::str::from_utf8(json_chunk)
            .expect("JSON UTF-8")
            .trim_end_matches(' ');
        let keys = [
            "\"asset\"",
            "\"scene\"",
            "\"scenes\"",
            "\"nodes\"",
            "\"meshes\"",
            "\"buffers\"",
            "\"bufferViews\"",
            "\"accessors\"",
        ];
        let offsets: Vec<_> = keys
            .iter()
            .map(|key| json.find(key).expect("fixed root property"))
            .collect();
        assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(json.contains("\"POSITION\":0,\"NORMAL\":1"));

        let binary_header = 20 + json_length;
        assert_eq!(
            read_u32_le_at(&bytes, binary_header + 4),
            Some(GLB_BIN_CHUNK_TYPE)
        );
        let binary_length =
            usize::try_from(read_u32_le_at(&bytes, binary_header).expect("BIN length"))
                .expect("usize BIN");
        assert_eq!(binary_length % 4, 0);
        assert_eq!(binary_header + 8 + binary_length, bytes.len());

        let mut bad_total = bytes.clone();
        bad_total[8..12].copy_from_slice(&0_u32.to_le_bytes());
        assert!(!verify_glb(&bad_total, &mesh, bad_total.len()));
        let mut bad_offset = bytes.clone();
        let offset_token = b"\"byteOffset\":96";
        let offset_start = bad_offset
            .windows(offset_token.len())
            .position(|window| window == offset_token)
            .expect("index buffer offset");
        bad_offset[offset_start + offset_token.len() - 1] = b'2';
        assert!(!verify_glb(&bad_offset, &mesh, bad_offset.len()));

        let mut bad_index = bytes.clone();
        let binary_start = binary_header + GLB_CHUNK_HEADER_BYTES;
        let index_start = binary_start + 4 * 3 * 2 * mesh.positions_mm.len();
        bad_index[index_start..index_start + 4].copy_from_slice(&3_u32.to_le_bytes());
        assert!(!verify_glb(&bad_index, &mesh, bad_index.len()));

        let mut bad_chunk_type = bytes;
        bad_chunk_type[16..20].copy_from_slice(&0_u32.to_le_bytes());
        assert!(!verify_glb(&bad_chunk_type, &mesh, bad_chunk_type.len()));
    }

    #[test]
    fn glb_node_rotation_maps_source_right_forward_up_without_reflection() {
        let transform = |vector: [f32; 3]| {
            [
                GLTF_NODE_MATRIX[0] * vector[0]
                    + GLTF_NODE_MATRIX[4] * vector[1]
                    + GLTF_NODE_MATRIX[8] * vector[2],
                GLTF_NODE_MATRIX[1] * vector[0]
                    + GLTF_NODE_MATRIX[5] * vector[1]
                    + GLTF_NODE_MATRIX[9] * vector[2],
                GLTF_NODE_MATRIX[2] * vector[0]
                    + GLTF_NODE_MATRIX[6] * vector[1]
                    + GLTF_NODE_MATRIX[10] * vector[2],
            ]
        };
        assert_eq!(transform([1.0, 0.0, 0.0]), [-1.0, 0.0, 0.0]);
        assert_eq!(transform([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(transform([0.0, 0.0, 1.0]), [0.0, 1.0, 0.0]);

        let first = transform([1.0, 0.0, 0.0]);
        let second = transform([0.0, 1.0, 0.0]);
        let third = transform([0.0, 0.0, 1.0]);
        let determinant = first[0] * (second[1] * third[2] - second[2] * third[1])
            - second[0] * (first[1] * third[2] - first[2] * third[1])
            + third[0] * (first[1] * second[2] - first[2] * second[1]);
        assert_eq!(determinant, 1.0);
    }

    #[test]
    fn all_formats_preserve_one_geometry_with_only_documented_unit_conversion() {
        let mesh = sample_mesh();
        let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
            .expect("OBJ")
            .bytes;
        let obj_text = std::str::from_utf8(&obj).expect("OBJ UTF-8");
        let obj_positions: Vec<[f64; 3]> = obj_text
            .lines()
            .filter_map(|line| line.strip_prefix("v "))
            .map(|line| {
                let values: Vec<f64> = line
                    .split(' ')
                    .map(|value| value.parse().expect("OBJ number"))
                    .collect();
                [values[0], values[1], values[2]]
            })
            .collect();
        assert_eq!(obj_positions, mesh.positions_mm);

        let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
            .expect("GLB")
            .bytes;
        let json_length =
            usize::try_from(read_u32_le_at(&glb, 12).expect("JSON length")).expect("usize");
        let binary_start = 20 + json_length + 8;
        let mut cursor = binary_start;
        for source in &mesh.positions_mm {
            for component in source {
                let encoded = read_f32_le(&glb, &mut cursor).expect("GLB position");
                assert_eq!(
                    encoded.to_bits(),
                    canonical_zero_f32((*component * 0.001) as f32).to_bits()
                );
            }
        }

        let stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh)
            .expect("STL")
            .bytes;
        let mut cursor = 84;
        for triangle in &mesh.triangles {
            cursor += 12;
            for index in triangle {
                for component in mesh.positions_mm[usize::try_from(*index).expect("usize index")] {
                    let encoded = read_f32_le(&stl, &mut cursor).expect("STL position");
                    assert_eq!(encoded.to_bits(), (component as f32).to_bits());
                }
            }
            cursor += 2;
        }
        assert_eq!(cursor, stl.len());
    }

    #[test]
    fn input_order_is_preserved_and_winding_changes_output_deterministically() {
        let first = sample_mesh();
        let mut reversed_document = sample_document();
        for triangle in &mut reversed_document.triangles {
            triangle.swap(1, 2);
        }
        let reversed =
            validate_indexed_triangle_mesh(&reversed_document).expect("reversed winding");
        for format in [
            StaticMeshExportFormat::Obj,
            StaticMeshExportFormat::BinaryStl,
            StaticMeshExportFormat::Glb20,
        ] {
            let first_bytes = export_static_triangle_mesh(format, &first)
                .expect("first")
                .bytes;
            let reversed_bytes = export_static_triangle_mesh(format, &reversed)
                .expect("reversed")
                .bytes;
            assert_ne!(first_bytes, reversed_bytes);
        }
    }

    #[test]
    fn public_format_metadata_is_fixed() {
        assert_eq!(StaticMeshExportFormat::Obj.media_type(), "model/obj");
        assert_eq!(StaticMeshExportFormat::Obj.file_extension(), "obj");
        assert_eq!(StaticMeshExportFormat::BinaryStl.media_type(), "model/stl");
        assert_eq!(StaticMeshExportFormat::BinaryStl.file_extension(), "stl");
        assert_eq!(
            StaticMeshExportFormat::Glb20.media_type(),
            "model/gltf-binary"
        );
        assert_eq!(StaticMeshExportFormat::Glb20.file_extension(), "glb");
    }
}
