//! Deterministic, bounded export of one static indexed triangle mesh.
//!
//! The admitted mesh uses millimetres and a right-handed, Z-up coordinate
//! system. OBJ and binary STL preserve those axes. GLB stores local positions
//! in metres and carries a fixed node rotation into glTF's Y-up scene axes.
//! This module deliberately has no project, current-pose, animation, texture,
//! staging, filesystem, or UI authority.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Version of the only indexed triangle mesh input schema accepted here.
pub const INDEXED_TRIANGLE_MESH_SCHEMA_VERSION_V1: u32 = 1;
/// Non-relaxable maximum number of indexed vertices.
pub const MAX_STATIC_MESH_VERTICES: usize = 100_000;
/// Non-relaxable maximum number of triangles.
pub const MAX_STATIC_MESH_TRIANGLES: usize = 200_000;
/// Non-relaxable maximum size of one exported file.
pub const MAX_STATIC_MESH_EXPORT_BYTES: usize = 64 * 1024 * 1024;
/// Non-relaxable size of one embedded GLB base-color image.
pub const MAX_STATIC_MESH_TEXTURE_BYTES: usize = 16 * 1024 * 1024;
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
const GLTF_UNSIGNED_BYTE: u32 = 5_121;
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
    #[serde(default = "opaque_white")]
    pub base_color_rgba: [u8; 4],
    /// Optional per-vertex sRGB colors. Empty means that the GLB material base
    /// color applies uniformly. OBJ and STL cannot preserve this channel.
    #[serde(default)]
    pub vertex_colors_rgba: Vec<[u8; 4]>,
    /// Optional embedded PNG/JPEG base-color texture and one UV per vertex.
    #[serde(default)]
    pub base_color_texture: Option<EmbeddedBaseColorTextureV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedBaseColorTextureV1 {
    pub media_type: EmbeddedTextureMediaTypeV1,
    pub bytes: Vec<u8>,
    pub tex_coords: Vec<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddedTextureMediaTypeV1 {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedSolidTriangleRegionV1 {
    FrontCap,
    BackCap,
    SideWall,
}

impl EmbeddedTextureMediaTypeV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
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
            base_color_rgba: opaque_white(),
            vertex_colors_rgba: Vec::new(),
            base_color_texture: None,
        }
    }

    #[must_use]
    pub const fn with_base_color_rgba(mut self, color: [u8; 4]) -> Self {
        self.base_color_rgba = color;
        self
    }

    #[must_use]
    pub fn with_vertex_colors_rgba(mut self, colors: Vec<[u8; 4]>) -> Self {
        self.vertex_colors_rgba = colors;
        self
    }

    #[must_use]
    pub fn with_base_color_texture(mut self, texture: EmbeddedBaseColorTextureV1) -> Self {
        self.base_color_texture = Some(texture);
        self
    }
}

const fn opaque_white() -> [u8; 4] {
    [255, 255, 255, 255]
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
    base_color_rgba: [u8; 4],
    vertex_colors_rgba: Vec<[u8; 4]>,
    base_color_texture: Option<EmbeddedBaseColorTextureV1>,
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

    #[must_use]
    pub const fn base_color_rgba(&self) -> [u8; 4] {
        self.base_color_rgba
    }

    #[must_use]
    pub fn vertex_colors_rgba(&self) -> &[[u8; 4]] {
        &self.vertex_colors_rgba
    }

    #[must_use]
    pub fn base_color_texture(&self) -> Option<&EmbeddedBaseColorTextureV1> {
        self.base_color_texture.as_ref()
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
    #[error("mesh has {actual} vertex colors; either zero or exactly {expected} are required")]
    VertexColorCountMismatch { actual: usize, expected: usize },
    #[error("texture has {actual} UV coordinates; exactly {expected} are required")]
    TextureCoordinateCountMismatch { actual: usize, expected: usize },
    #[error("texture is {actual} bytes; the limit is {maximum} bytes")]
    TextureTooLarge { actual: usize, maximum: usize },
    #[error("texture payload does not match its declared media type")]
    InvalidTexturePayload,
    #[error("texture coordinate {vertex_index} is non-finite")]
    NonFiniteTextureCoordinate { vertex_index: usize },
    #[error("closed-solid textured export requires a front texture")]
    MissingFrontTexture,
    #[error("triangle-region count must equal triangle count")]
    TriangleRegionCountMismatch,
    #[error("closed-solid region classification must contain front, back, and side triangles")]
    IncompleteTriangleRegionCoverage,
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
    if !document.vertex_colors_rgba.is_empty()
        && document.vertex_colors_rgba.len() != document.positions_mm.len()
    {
        return Err(StaticMeshExportError::VertexColorCountMismatch {
            actual: document.vertex_colors_rgba.len(),
            expected: document.positions_mm.len(),
        });
    }
    if let Some(texture) = &document.base_color_texture {
        if texture.bytes.len() > MAX_STATIC_MESH_TEXTURE_BYTES {
            return Err(StaticMeshExportError::TextureTooLarge {
                actual: texture.bytes.len(),
                maximum: MAX_STATIC_MESH_TEXTURE_BYTES,
            });
        }
        if texture.tex_coords.len() != document.positions_mm.len() {
            return Err(StaticMeshExportError::TextureCoordinateCountMismatch {
                actual: texture.tex_coords.len(),
                expected: document.positions_mm.len(),
            });
        }
        let valid_payload = match texture.media_type {
            EmbeddedTextureMediaTypeV1::Png => texture.bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
            EmbeddedTextureMediaTypeV1::Jpeg => {
                texture.bytes.starts_with(&[0xff, 0xd8]) && texture.bytes.ends_with(&[0xff, 0xd9])
            }
        };
        if !valid_payload {
            return Err(StaticMeshExportError::InvalidTexturePayload);
        }
        if let Some(vertex_index) = texture
            .tex_coords
            .iter()
            .position(|uv| uv.iter().any(|value| !value.is_finite()))
        {
            return Err(StaticMeshExportError::NonFiniteTextureCoordinate { vertex_index });
        }
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
        base_color_rgba: document.base_color_rgba,
        vertex_colors_rgba: document.vertex_colors_rgba.clone(),
        base_color_texture: document.base_color_texture.clone(),
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

/// Exports a zero-thickness sheet as two explicitly oriented glTF
/// primitives. The existing static exporter remains byte-for-byte unchanged.
pub fn export_dual_sided_triangle_mesh_glb(
    mesh: &ValidatedIndexedTriangleMesh,
    back_texture: EmbeddedBaseColorTextureV1,
    back_base_color_rgba: [u8; 4],
) -> Result<StaticMeshExportArtifact, StaticMeshExportError> {
    export_dual_sided_triangle_mesh_glb_with_limits(
        mesh,
        back_texture,
        back_base_color_rgba,
        StaticMeshExportLimits::default(),
    )
}

pub fn export_dual_sided_triangle_mesh_glb_with_limits(
    mesh: &ValidatedIndexedTriangleMesh,
    back_texture: EmbeddedBaseColorTextureV1,
    back_base_color_rgba: [u8; 4],
    limits: StaticMeshExportLimits,
) -> Result<StaticMeshExportArtifact, StaticMeshExportError> {
    validate_embedded_texture(&back_texture, mesh.positions_mm.len())?;
    let front = serialize_glb(
        mesh,
        limits.max_output_bytes.min(MAX_STATIC_MESH_EXPORT_BYTES),
    )?;
    let bytes = append_back_primitive_glb(
        &front,
        mesh,
        &back_texture,
        back_base_color_rgba,
        limits.max_output_bytes.min(MAX_STATIC_MESH_EXPORT_BYTES),
    )?;
    Ok(StaticMeshExportArtifact {
        format: StaticMeshExportFormat::Glb20,
        media_type: StaticMeshExportFormat::Glb20.media_type(),
        file_extension: StaticMeshExportFormat::Glb20.file_extension(),
        bytes,
        vertex_count: mesh.positions_mm.len(),
        triangle_count: mesh.triangles.len() * 2,
    })
}

pub fn export_regioned_closed_solid_triangle_mesh_glb(
    mesh: &ValidatedIndexedTriangleMesh,
    triangle_regions: &[ClosedSolidTriangleRegionV1],
    back_texture: EmbeddedBaseColorTextureV1,
    back_base_color_rgba: [u8; 4],
    side_base_color_rgba: [u8; 4],
) -> Result<StaticMeshExportArtifact, StaticMeshExportError> {
    export_regioned_closed_solid_triangle_mesh_glb_with_limits(
        mesh,
        triangle_regions,
        back_texture,
        back_base_color_rgba,
        side_base_color_rgba,
        StaticMeshExportLimits::default(),
    )
}

pub fn export_regioned_closed_solid_triangle_mesh_glb_with_limits(
    mesh: &ValidatedIndexedTriangleMesh,
    triangle_regions: &[ClosedSolidTriangleRegionV1],
    back_texture: EmbeddedBaseColorTextureV1,
    back_base_color_rgba: [u8; 4],
    side_base_color_rgba: [u8; 4],
    limits: StaticMeshExportLimits,
) -> Result<StaticMeshExportArtifact, StaticMeshExportError> {
    if mesh.base_color_texture.is_none() {
        return Err(StaticMeshExportError::MissingFrontTexture);
    }
    if triangle_regions.len() != mesh.triangles.len() {
        return Err(StaticMeshExportError::TriangleRegionCountMismatch);
    }
    if ![
        ClosedSolidTriangleRegionV1::FrontCap,
        ClosedSolidTriangleRegionV1::BackCap,
        ClosedSolidTriangleRegionV1::SideWall,
    ]
    .iter()
    .all(|region| triangle_regions.contains(region))
    {
        return Err(StaticMeshExportError::IncompleteTriangleRegionCoverage);
    }
    validate_embedded_texture(&back_texture, mesh.positions_mm.len())?;
    let maximum = limits.max_output_bytes.min(MAX_STATIC_MESH_EXPORT_BYTES);
    let front = serialize_glb(mesh, maximum)?;
    let bytes = region_closed_solid_glb(
        &front,
        mesh,
        triangle_regions,
        &back_texture,
        back_base_color_rgba,
        side_base_color_rgba,
        maximum,
    )?;
    Ok(StaticMeshExportArtifact {
        format: StaticMeshExportFormat::Glb20,
        media_type: StaticMeshExportFormat::Glb20.media_type(),
        file_extension: StaticMeshExportFormat::Glb20.file_extension(),
        bytes,
        vertex_count: mesh.positions_mm.len(),
        triangle_count: mesh.triangles.len(),
    })
}

fn validate_embedded_texture(
    texture: &EmbeddedBaseColorTextureV1,
    vertex_count: usize,
) -> Result<(), StaticMeshExportError> {
    if texture.bytes.len() > MAX_STATIC_MESH_TEXTURE_BYTES {
        return Err(StaticMeshExportError::TextureTooLarge {
            actual: texture.bytes.len(),
            maximum: MAX_STATIC_MESH_TEXTURE_BYTES,
        });
    }
    if texture.tex_coords.len() != vertex_count {
        return Err(StaticMeshExportError::TextureCoordinateCountMismatch {
            actual: texture.tex_coords.len(),
            expected: vertex_count,
        });
    }
    let payload_valid = match texture.media_type {
        EmbeddedTextureMediaTypeV1::Png => texture.bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        EmbeddedTextureMediaTypeV1::Jpeg => {
            texture.bytes.starts_with(&[0xff, 0xd8]) && texture.bytes.ends_with(&[0xff, 0xd9])
        }
    };
    if !payload_valid {
        return Err(StaticMeshExportError::InvalidTexturePayload);
    }
    if let Some(vertex_index) = texture
        .tex_coords
        .iter()
        .position(|uv| uv.iter().any(|value| !value.is_finite()))
    {
        return Err(StaticMeshExportError::NonFiniteTextureCoordinate { vertex_index });
    }
    Ok(())
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
    materials: [GlbMaterial; 1],
    buffers: [GlbBuffer; 1],
    #[serde(rename = "bufferViews")]
    buffer_views: Vec<GlbBufferView>,
    accessors: Vec<GlbAccessor>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<GlbImage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    textures: Vec<GlbTexture>,
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
    material: u32,
}

#[derive(Serialize)]
struct GlbMaterial {
    name: &'static str,
    #[serde(rename = "pbrMetallicRoughness")]
    pbr: GlbPbrMaterial,
    #[serde(rename = "doubleSided")]
    double_sided: bool,
}

#[derive(Serialize)]
struct GlbPbrMaterial {
    #[serde(rename = "baseColorFactor")]
    base_color_factor: [f32; 4],
    #[serde(rename = "metallicFactor")]
    metallic_factor: f32,
    #[serde(rename = "roughnessFactor")]
    roughness_factor: f32,
    #[serde(rename = "baseColorTexture", skip_serializing_if = "Option::is_none")]
    base_color_texture: Option<GlbTextureInfo>,
}

#[derive(Serialize)]
struct GlbTextureInfo {
    index: u32,
    #[serde(rename = "texCoord")]
    tex_coord: u32,
}
#[derive(Serialize)]
struct GlbImage {
    #[serde(rename = "bufferView")]
    buffer_view: u32,
    #[serde(rename = "mimeType")]
    mime_type: &'static str,
}
#[derive(Serialize)]
struct GlbTexture {
    source: u32,
}

#[derive(Serialize)]
struct GlbAttributes {
    #[serde(rename = "POSITION")]
    position: u32,
    #[serde(rename = "NORMAL")]
    normal: u32,
    #[serde(rename = "COLOR_0", skip_serializing_if = "Option::is_none")]
    color: Option<u32>,
    #[serde(rename = "TEXCOORD_0", skip_serializing_if = "Option::is_none")]
    tex_coord: Option<u32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<u32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    min: Option<GlbAccessorBounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max: Option<GlbAccessorBounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized: Option<bool>,
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
    #[serde(rename = "extensionsUsed", default)]
    _extensions_used: Vec<String>,
    #[serde(rename = "extensions", default)]
    _extensions: serde_json::Map<String, serde_json::Value>,
    asset: CheckedGlbAsset,
    scene: u32,
    scenes: Vec<CheckedGlbScene>,
    nodes: Vec<CheckedGlbNode>,
    meshes: Vec<CheckedGlbMesh>,
    materials: Vec<CheckedGlbMaterial>,
    buffers: Vec<CheckedGlbBuffer>,
    #[serde(rename = "bufferViews")]
    buffer_views: Vec<CheckedGlbBufferView>,
    accessors: Vec<CheckedGlbAccessor>,
    #[serde(default)]
    images: Vec<CheckedGlbImage>,
    #[serde(default)]
    textures: Vec<CheckedGlbTexture>,
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
    material: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbMaterial {
    name: String,
    #[serde(rename = "pbrMetallicRoughness")]
    pbr: CheckedGlbPbrMaterial,
    #[serde(rename = "doubleSided")]
    double_sided: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbPbrMaterial {
    #[serde(rename = "baseColorFactor")]
    base_color_factor: [f32; 4],
    #[serde(rename = "metallicFactor")]
    metallic_factor: f32,
    #[serde(rename = "roughnessFactor")]
    roughness_factor: f32,
    #[serde(rename = "baseColorTexture", default)]
    base_color_texture: Option<CheckedGlbTextureInfo>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbTextureInfo {
    index: u32,
    #[serde(rename = "texCoord")]
    tex_coord: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbImage {
    #[serde(rename = "bufferView")]
    buffer_view: u32,
    #[serde(rename = "mimeType")]
    mime_type: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbTexture {
    source: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckedGlbAttributes {
    #[serde(rename = "POSITION")]
    position: u32,
    #[serde(rename = "NORMAL")]
    normal: u32,
    #[serde(rename = "COLOR_0", default)]
    color: Option<u32>,
    #[serde(rename = "TEXCOORD_0", default)]
    tex_coord: Option<u32>,
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
    #[serde(default)]
    target: Option<u32>,
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
    #[serde(default)]
    min: Option<Vec<serde_json::Number>>,
    #[serde(default)]
    max: Option<Vec<serde_json::Number>>,
    #[serde(default)]
    normalized: Option<bool>,
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
    let color_bytes = mesh.vertex_colors_rgba.len().checked_mul(4).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
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
    let texture = mesh.base_color_texture.as_ref();
    let uv_bytes = texture.map_or(0, |value| value.tex_coords.len() * 8);
    let normal_offset = position_bytes;
    let color_offset = normal_offset.checked_add(normal_bytes).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let index_offset = color_offset.checked_add(color_bytes).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let uv_offset = index_offset.checked_add(index_bytes).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let image_offset = uv_offset.checked_add(uv_bytes).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let image_length = texture.map_or(0, |value| value.bytes.len());
    let binary_length = align4(image_offset.checked_add(image_length).ok_or(
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        },
    )?)
    .ok_or(StaticMeshExportError::StructureNotRepresentable {
        format: StaticMeshExportFormat::Glb20,
    })?;
    if normal_offset % 4 != 0
        || color_offset % 4 != 0
        || index_offset % 4 != 0
        || binary_length % 4 != 0
    {
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
    let color_accessor = (!mesh.vertex_colors_rgba.is_empty()).then_some(3_u32);
    let mut buffer_views = vec![
        GlbBufferView {
            buffer: 0,
            byte_offset: 0,
            byte_length: position_bytes,
            target: Some(GLTF_ARRAY_BUFFER),
        },
        GlbBufferView {
            buffer: 0,
            byte_offset: normal_offset,
            byte_length: normal_bytes,
            target: Some(GLTF_ARRAY_BUFFER),
        },
        GlbBufferView {
            buffer: 0,
            byte_offset: index_offset,
            byte_length: index_bytes,
            target: Some(GLTF_ELEMENT_ARRAY_BUFFER),
        },
    ];
    let mut accessors = vec![
        GlbAccessor {
            buffer_view: 0,
            byte_offset: 0,
            component_type: GLTF_FLOAT,
            count: positions.len(),
            accessor_type: "VEC3",
            min: Some(GlbAccessorBounds::Vec3(position_min)),
            max: Some(GlbAccessorBounds::Vec3(position_max)),
            normalized: None,
        },
        GlbAccessor {
            buffer_view: 1,
            byte_offset: 0,
            component_type: GLTF_FLOAT,
            count: normals.len(),
            accessor_type: "VEC3",
            min: Some(GlbAccessorBounds::Vec3(normal_min)),
            max: Some(GlbAccessorBounds::Vec3(normal_max)),
            normalized: None,
        },
        GlbAccessor {
            buffer_view: 2,
            byte_offset: 0,
            component_type: GLTF_UNSIGNED_INT,
            count: index_count,
            accessor_type: "SCALAR",
            min: Some(GlbAccessorBounds::Scalar([index_min])),
            max: Some(GlbAccessorBounds::Scalar([index_max])),
            normalized: None,
        },
    ];
    if color_accessor.is_some() {
        buffer_views.push(GlbBufferView {
            buffer: 0,
            byte_offset: color_offset,
            byte_length: color_bytes,
            target: Some(GLTF_ARRAY_BUFFER),
        });
        accessors.push(GlbAccessor {
            buffer_view: 3,
            byte_offset: 0,
            component_type: GLTF_UNSIGNED_BYTE,
            count: mesh.vertex_colors_rgba.len(),
            accessor_type: "VEC4",
            min: None,
            max: None,
            normalized: Some(true),
        });
    }
    let uv_accessor = texture.map(|_| accessors.len() as u32);
    if let Some(texture) = texture {
        buffer_views.push(GlbBufferView {
            buffer: 0,
            byte_offset: uv_offset,
            byte_length: uv_bytes,
            target: Some(GLTF_ARRAY_BUFFER),
        });
        accessors.push(GlbAccessor {
            buffer_view: (buffer_views.len() - 1) as u32,
            byte_offset: 0,
            component_type: GLTF_FLOAT,
            count: texture.tex_coords.len(),
            accessor_type: "VEC2",
            min: None,
            max: None,
            normalized: None,
        });
        buffer_views.push(GlbBufferView {
            buffer: 0,
            byte_offset: image_offset,
            byte_length: image_length,
            target: None,
        });
    }
    let image_view = texture.map(|_| (buffer_views.len() - 1) as u32);
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
                    color: color_accessor,
                    tex_coord: uv_accessor,
                },
                indices: 2,
                mode: GLTF_TRIANGLES,
                material: 0,
            }],
        }],
        materials: [GlbMaterial {
            name: "ORIGAMI2 Paper",
            pbr: GlbPbrMaterial {
                base_color_factor: mesh
                    .base_color_rgba
                    .map(|channel| f32::from(channel) / 255.0),
                metallic_factor: 0.0,
                roughness_factor: 1.0,
                base_color_texture: texture.map(|_| GlbTextureInfo {
                    index: 0,
                    tex_coord: 0,
                }),
            },
            double_sided: true,
        }],
        buffers: [GlbBuffer {
            byte_length: binary_length,
        }],
        buffer_views,
        accessors,
        images: texture
            .map(|value| {
                vec![GlbImage {
                    buffer_view: image_view.expect("texture has image view"),
                    mime_type: value.media_type.as_str(),
                }]
            })
            .unwrap_or_default(),
        textures: texture
            .map(|_| vec![GlbTexture { source: 0 }])
            .unwrap_or_default(),
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
    for color in &mesh.vertex_colors_rgba {
        output.extend_from_slice(color);
    }
    for triangle in &mesh.triangles {
        for index in triangle {
            output.extend_from_slice(&index.to_le_bytes());
        }
    }
    if let Some(texture) = texture {
        for uv in &texture.tex_coords {
            for component in uv {
                output.extend_from_slice(&component.to_le_bytes());
            }
        }
        output.extend_from_slice(&texture.bytes);
        output.resize(total_length, 0);
    }
    debug_assert_eq!(output.len(), total_length);
    Ok(output)
}

fn append_back_primitive_glb(
    front: &[u8],
    mesh: &ValidatedIndexedTriangleMesh,
    texture: &EmbeddedBaseColorTextureV1,
    back_color: [u8; 4],
    maximum: usize,
) -> Result<Vec<u8>, StaticMeshExportError> {
    let fail = || StaticMeshExportError::StructureNotRepresentable {
        format: StaticMeshExportFormat::Glb20,
    };
    let json_len = read_u32_le_at(front, 12)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(fail)?;
    let json_start = 20usize;
    let bin_header = json_start.checked_add(json_len).ok_or_else(fail)?;
    let bin_len = read_u32_le_at(front, bin_header)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(fail)?;
    if read_u32_le_at(front, bin_header + 4) != Some(GLB_BIN_CHUNK_TYPE) {
        return Err(fail());
    }
    let bin_start = bin_header + 8;
    let mut binary = front
        .get(bin_start..bin_start.checked_add(bin_len).ok_or_else(fail)?)
        .ok_or_else(fail)?
        .to_vec();
    let mut root: serde_json::Value =
        serde_json::from_slice(front.get(json_start..bin_header).ok_or_else(fail)?)
            .map_err(|_| fail())?;

    let normal_view = root["bufferViews"].as_array().ok_or_else(fail)?.len();
    let normal_offset = binary.len();
    for normal in &mesh.normals {
        for component in normal {
            binary.extend_from_slice(&canonical_zero_f32(-(*component as f32)).to_le_bytes());
        }
    }
    root["bufferViews"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "buffer": 0, "byteOffset": normal_offset,
            "byteLength": mesh.normals.len() * 12, "target": GLTF_ARRAY_BUFFER
        }));
    let normal_accessor = root["accessors"].as_array().ok_or_else(fail)?.len();
    root["accessors"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "bufferView": normal_view, "byteOffset": 0, "componentType": GLTF_FLOAT,
            "count": mesh.normals.len(), "type": "VEC3"
        }));

    let index_view = root["bufferViews"].as_array().ok_or_else(fail)?.len();
    let index_offset = binary.len();
    for triangle in &mesh.triangles {
        for index in [triangle[0], triangle[2], triangle[1]] {
            binary.extend_from_slice(&index.to_le_bytes());
        }
    }
    root["bufferViews"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "buffer": 0, "byteOffset": index_offset,
            "byteLength": mesh.triangles.len() * 12, "target": GLTF_ELEMENT_ARRAY_BUFFER
        }));
    let index_accessor = root["accessors"].as_array().ok_or_else(fail)?.len();
    root["accessors"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "bufferView": index_view, "byteOffset": 0, "componentType": GLTF_UNSIGNED_INT,
            "count": mesh.triangles.len() * 3, "type": "SCALAR"
        }));

    let uv_view = root["bufferViews"].as_array().ok_or_else(fail)?.len();
    let uv_offset = binary.len();
    for uv in &texture.tex_coords {
        for component in uv {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    root["bufferViews"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "buffer": 0, "byteOffset": uv_offset,
            "byteLength": texture.tex_coords.len() * 8, "target": GLTF_ARRAY_BUFFER
        }));
    let uv_accessor = root["accessors"].as_array().ok_or_else(fail)?.len();
    root["accessors"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "bufferView": uv_view, "byteOffset": 0, "componentType": GLTF_FLOAT,
            "count": texture.tex_coords.len(), "type": "VEC2"
        }));

    let image_view = root["bufferViews"].as_array().ok_or_else(fail)?.len();
    let image_offset = binary.len();
    binary.extend_from_slice(&texture.bytes);
    root["bufferViews"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "buffer": 0, "byteOffset": image_offset, "byteLength": texture.bytes.len()
        }));
    while binary.len() % 4 != 0 {
        binary.push(0);
    }

    let images = root["images"].as_array_mut().ok_or_else(fail)?;
    let image_index = images.len();
    images.push(serde_json::json!({
        "bufferView": image_view, "mimeType": texture.media_type.as_str()
    }));
    let textures = root["textures"].as_array_mut().ok_or_else(fail)?;
    let texture_index = textures.len();
    textures.push(serde_json::json!({"source": image_index}));
    let materials = root["materials"].as_array_mut().ok_or_else(fail)?;
    let material_index = materials.len();
    materials[0]["doubleSided"] = serde_json::Value::Bool(false);
    materials.push(serde_json::json!({
        "name": "ORIGAMI2 Paper Back",
        "pbrMetallicRoughness": {
            "baseColorFactor": back_color.map(|channel| f32::from(channel) / 255.0),
            "metallicFactor": 0.0, "roughnessFactor": 1.0,
            "baseColorTexture": {"index": texture_index, "texCoord": 0}
        },
        "doubleSided": false
    }));
    let primitives = root["meshes"][0]["primitives"]
        .as_array_mut()
        .ok_or_else(fail)?;
    let mut back = primitives.first().cloned().ok_or_else(fail)?;
    back["attributes"]["NORMAL"] = serde_json::json!(normal_accessor);
    back["attributes"]["TEXCOORD_0"] = serde_json::json!(uv_accessor);
    back["indices"] = serde_json::json!(index_accessor);
    back["material"] = serde_json::json!(material_index);
    primitives.push(back);
    root["buffers"][0]["byteLength"] = serde_json::json!(binary.len());

    let json = serde_json::to_vec(&root).map_err(|_| fail())?;
    if json.len() > MAX_GLB_JSON_BYTES {
        return Err(fail());
    }
    let json_padded = align4(json.len()).ok_or_else(fail)?;
    let total = 12usize
        .checked_add(8)
        .and_then(|value| value.checked_add(json_padded))
        .and_then(|value| value.checked_add(8))
        .and_then(|value| value.checked_add(binary.len()))
        .ok_or_else(fail)?;
    if total > maximum {
        return Err(StaticMeshExportError::OutputTooLarge {
            actual: total,
            maximum,
        });
    }
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(total).map_err(|_| fail())?.to_le_bytes());
    output.extend_from_slice(
        &u32::try_from(json_padded)
            .map_err(|_| fail())?
            .to_le_bytes(),
    );
    output.extend_from_slice(&GLB_JSON_CHUNK_TYPE.to_le_bytes());
    output.extend_from_slice(&json);
    output.resize(20 + json_padded, b' ');
    output.extend_from_slice(
        &u32::try_from(binary.len())
            .map_err(|_| fail())?
            .to_le_bytes(),
    );
    output.extend_from_slice(&GLB_BIN_CHUNK_TYPE.to_le_bytes());
    output.extend_from_slice(&binary);
    Ok(output)
}

fn region_closed_solid_glb(
    front: &[u8],
    mesh: &ValidatedIndexedTriangleMesh,
    regions: &[ClosedSolidTriangleRegionV1],
    back_texture: &EmbeddedBaseColorTextureV1,
    back_color: [u8; 4],
    side_color: [u8; 4],
    maximum: usize,
) -> Result<Vec<u8>, StaticMeshExportError> {
    let fail = || StaticMeshExportError::StructureNotRepresentable {
        format: StaticMeshExportFormat::Glb20,
    };
    let json_len = read_u32_le_at(front, 12)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(fail)?;
    let bin_header = 20usize.checked_add(json_len).ok_or_else(fail)?;
    let bin_len = read_u32_le_at(front, bin_header)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(fail)?;
    let bin_start = bin_header.checked_add(8).ok_or_else(fail)?;
    let mut binary = front
        .get(bin_start..bin_start.checked_add(bin_len).ok_or_else(fail)?)
        .ok_or_else(fail)?
        .to_vec();
    let mut root: serde_json::Value =
        serde_json::from_slice(front.get(20..bin_header).ok_or_else(fail)?).map_err(|_| fail())?;

    let mut region_accessors = Vec::new();
    for region in [
        ClosedSolidTriangleRegionV1::FrontCap,
        ClosedSolidTriangleRegionV1::BackCap,
        ClosedSolidTriangleRegionV1::SideWall,
    ] {
        let selected = mesh
            .triangles
            .iter()
            .zip(regions)
            .filter_map(|(triangle, actual)| (*actual == region).then_some(*triangle))
            .collect::<Vec<_>>();
        let view = root["bufferViews"].as_array().ok_or_else(fail)?.len();
        let offset = binary.len();
        for triangle in &selected {
            for index in triangle {
                binary.extend_from_slice(&index.to_le_bytes());
            }
        }
        root["bufferViews"]
            .as_array_mut()
            .ok_or_else(fail)?
            .push(serde_json::json!({
                "buffer":0, "byteOffset":offset, "byteLength":selected.len() * 12,
                "target":GLTF_ELEMENT_ARRAY_BUFFER
            }));
        let accessor = root["accessors"].as_array().ok_or_else(fail)?.len();
        root["accessors"]
            .as_array_mut()
            .ok_or_else(fail)?
            .push(serde_json::json!({
                "bufferView":view, "byteOffset":0, "componentType":GLTF_UNSIGNED_INT,
                "count":selected.len() * 3, "type":"SCALAR"
            }));
        region_accessors.push(accessor);
    }

    let uv_view = root["bufferViews"].as_array().ok_or_else(fail)?.len();
    let uv_offset = binary.len();
    for uv in &back_texture.tex_coords {
        for component in uv {
            binary.extend_from_slice(&component.to_le_bytes());
        }
    }
    root["bufferViews"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "buffer":0, "byteOffset":uv_offset,
            "byteLength":back_texture.tex_coords.len() * 8, "target":GLTF_ARRAY_BUFFER
        }));
    let uv_accessor = root["accessors"].as_array().ok_or_else(fail)?.len();
    root["accessors"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "bufferView":uv_view, "byteOffset":0, "componentType":GLTF_FLOAT,
            "count":back_texture.tex_coords.len(), "type":"VEC2"
        }));
    let image_view = root["bufferViews"].as_array().ok_or_else(fail)?.len();
    let image_offset = binary.len();
    binary.extend_from_slice(&back_texture.bytes);
    root["bufferViews"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "buffer":0, "byteOffset":image_offset, "byteLength":back_texture.bytes.len()
        }));
    while binary.len() % 4 != 0 {
        binary.push(0);
    }
    let image_index = root["images"].as_array().ok_or_else(fail)?.len();
    root["images"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({
            "bufferView":image_view, "mimeType":back_texture.media_type.as_str()
        }));
    let texture_index = root["textures"].as_array().ok_or_else(fail)?.len();
    root["textures"]
        .as_array_mut()
        .ok_or_else(fail)?
        .push(serde_json::json!({"source":image_index}));
    let materials = root["materials"].as_array_mut().ok_or_else(fail)?;
    materials[0]["doubleSided"] = serde_json::json!(false);
    materials.push(serde_json::json!({
        "name":"ORIGAMI2 Paper Back",
        "pbrMetallicRoughness":{
            "baseColorFactor":back_color.map(|channel| f32::from(channel)/255.0),
            "metallicFactor":0.0,"roughnessFactor":1.0,
            "baseColorTexture":{"index":texture_index,"texCoord":0}
        },"doubleSided":false
    }));
    materials.push(serde_json::json!({
        "name":"ORIGAMI2 Paper Edge",
        "pbrMetallicRoughness":{
            "baseColorFactor":side_color.map(|channel| f32::from(channel)/255.0),
            "metallicFactor":0.0,"roughnessFactor":1.0
        },"doubleSided":false
    }));
    let primitives = root["meshes"][0]["primitives"]
        .as_array_mut()
        .ok_or_else(fail)?;
    let template = primitives.first().cloned().ok_or_else(fail)?;
    let mut front_primitive = template.clone();
    front_primitive["indices"] = serde_json::json!(region_accessors[0]);
    let mut back_primitive = template.clone();
    back_primitive["indices"] = serde_json::json!(region_accessors[1]);
    back_primitive["attributes"]["TEXCOORD_0"] = serde_json::json!(uv_accessor);
    back_primitive["material"] = serde_json::json!(1);
    let mut side_primitive = template;
    side_primitive["indices"] = serde_json::json!(region_accessors[2]);
    side_primitive["material"] = serde_json::json!(2);
    side_primitive["attributes"]
        .as_object_mut()
        .ok_or_else(fail)?
        .remove("TEXCOORD_0");
    *primitives = vec![front_primitive, back_primitive, side_primitive];
    root["buffers"][0]["byteLength"] = serde_json::json!(binary.len());
    encode_glb_root_and_binary(&root, &binary, maximum)
}

fn encode_glb_root_and_binary(
    root: &serde_json::Value,
    binary: &[u8],
    maximum: usize,
) -> Result<Vec<u8>, StaticMeshExportError> {
    let fail = || StaticMeshExportError::StructureNotRepresentable {
        format: StaticMeshExportFormat::Glb20,
    };
    let json = serde_json::to_vec(root).map_err(|_| fail())?;
    if json.len() > MAX_GLB_JSON_BYTES || !binary.len().is_multiple_of(4) {
        return Err(fail());
    }
    let padded = align4(json.len()).ok_or_else(fail)?;
    let total = 28usize
        .checked_add(padded)
        .and_then(|value| value.checked_add(binary.len()))
        .ok_or_else(fail)?;
    if total > maximum {
        return Err(StaticMeshExportError::OutputTooLarge {
            actual: total,
            maximum,
        });
    }
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(b"glTF");
    output.extend_from_slice(&2_u32.to_le_bytes());
    output.extend_from_slice(&u32::try_from(total).map_err(|_| fail())?.to_le_bytes());
    output.extend_from_slice(&u32::try_from(padded).map_err(|_| fail())?.to_le_bytes());
    output.extend_from_slice(&GLB_JSON_CHUNK_TYPE.to_le_bytes());
    output.extend_from_slice(&json);
    output.resize(20 + padded, b' ');
    output.extend_from_slice(
        &u32::try_from(binary.len())
            .map_err(|_| fail())?
            .to_le_bytes(),
    );
    output.extend_from_slice(&GLB_BIN_CHUNK_TYPE.to_le_bytes());
    output.extend_from_slice(binary);
    Ok(output)
}

const ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1: &str = "ORIGAMI2_generation_provenance_v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GlbGenerationProvenanceExtensionV1 {
    version: u32,
    provenance: ori_domain::BeginnerGenerationProvenanceV1,
    provenance_sha256: [u8; 32],
}

/// Exports GLB 2.0 with one optional, non-required ORIGAMI2 provenance extension.
pub fn export_static_triangle_mesh_glb_with_provenance(
    mesh: &ValidatedIndexedTriangleMesh,
    provenance: &ori_domain::BeginnerGenerationProvenanceV1,
) -> Result<StaticMeshExportArtifact, StaticMeshExportError> {
    if !ori_domain::validate_beginner_generation_provenance_v1(provenance) {
        return Err(StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        });
    }
    let plain = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, mesh)?;
    let json_length = read_u32_le_at(&plain.bytes, 12)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(StaticMeshExportError::InternalVerificationFailed {
            format: StaticMeshExportFormat::Glb20,
        })?;
    let json_end = 20usize.checked_add(json_length).ok_or(
        StaticMeshExportError::InternalVerificationFailed {
            format: StaticMeshExportFormat::Glb20,
        },
    )?;
    let binary_header = json_end;
    let binary_length = read_u32_le_at(&plain.bytes, binary_header)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(StaticMeshExportError::InternalVerificationFailed {
            format: StaticMeshExportFormat::Glb20,
        })?;
    let binary_start = binary_header + 8;
    let binary_end = binary_start + binary_length;
    let mut root: serde_json::Value =
        serde_json::from_slice(&plain.bytes[20..json_end]).map_err(|_| {
            StaticMeshExportError::InternalVerificationFailed {
                format: StaticMeshExportFormat::Glb20,
            }
        })?;
    let provenance_bytes = serde_json::to_vec(provenance).map_err(|_| {
        StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        }
    })?;
    root["extensionsUsed"] = serde_json::json!([ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1]);
    root["extensions"][ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1] =
        serde_json::to_value(GlbGenerationProvenanceExtensionV1 {
            version: 1,
            provenance: provenance.clone(),
            provenance_sha256: Sha256::digest(provenance_bytes).into(),
        })
        .map_err(|_| StaticMeshExportError::StructureNotRepresentable {
            format: StaticMeshExportFormat::Glb20,
        })?;
    let bytes = encode_glb_root_and_binary(
        &root,
        plain.bytes.get(binary_start..binary_end).ok_or(
            StaticMeshExportError::InternalVerificationFailed {
                format: StaticMeshExportFormat::Glb20,
            },
        )?,
        MAX_STATIC_MESH_EXPORT_BYTES,
    )?;
    if !verify_glb(&bytes, mesh, MAX_STATIC_MESH_EXPORT_BYTES) {
        return Err(StaticMeshExportError::InternalVerificationFailed {
            format: StaticMeshExportFormat::Glb20,
        });
    }
    Ok(StaticMeshExportArtifact { bytes, ..plain })
}

/// Reads and independently authenticates the optional ORIGAMI2 GLB extension.
pub fn read_glb_generation_provenance(
    bytes: &[u8],
) -> Result<Option<ori_domain::BeginnerGenerationProvenanceV1>, StaticMeshExportError> {
    let fail = || StaticMeshExportError::StructureNotRepresentable {
        format: StaticMeshExportFormat::Glb20,
    };
    if bytes.len() < 28 || bytes.get(..4) != Some(b"glTF") || read_u32_le_at(bytes, 4) != Some(2) {
        return Err(fail());
    }
    let json_length = read_u32_le_at(bytes, 12)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|length| *length <= MAX_GLB_JSON_BYTES)
        .ok_or_else(fail)?;
    let json_end = 20usize.checked_add(json_length).ok_or_else(fail)?;
    let root: serde_json::Value =
        serde_json::from_slice(bytes.get(20..json_end).ok_or_else(fail)?).map_err(|_| fail())?;
    let used = root
        .get("extensionsUsed")
        .and_then(serde_json::Value::as_array);
    let value = root
        .get("extensions")
        .and_then(serde_json::Value::as_object)
        .and_then(|extensions| extensions.get(ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1));
    if value.is_none() {
        return if used.is_some_and(|items| {
            items
                .iter()
                .any(|item| item.as_str() == Some(ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1))
        }) {
            Err(fail())
        } else {
            Ok(None)
        };
    }
    if used.is_none_or(|items| {
        items
            .iter()
            .filter(|item| item.as_str() == Some(ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1))
            .count()
            != 1
    }) {
        return Err(fail());
    }
    let extension: GlbGenerationProvenanceExtensionV1 =
        serde_json::from_value(value.cloned().ok_or_else(fail)?).map_err(|_| fail())?;
    let encoded = serde_json::to_vec(&extension.provenance).map_err(|_| fail())?;
    if extension.version != 1
        || extension.provenance_sha256 != <[u8; 32]>::from(Sha256::digest(encoded))
        || !ori_domain::validate_beginner_generation_provenance_v1(&extension.provenance)
    {
        return Err(fail());
    }
    Ok(Some(extension.provenance))
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
        || root.meshes[0].primitives[0].attributes.color
            != (!mesh.vertex_colors_rgba.is_empty()).then_some(3)
        || root.meshes[0].primitives[0].attributes.tex_coord
            != mesh.base_color_texture.as_ref().map(|_| {
                if mesh.vertex_colors_rgba.is_empty() {
                    3
                } else {
                    4
                }
            })
        || root.meshes[0].primitives[0].indices != 2
        || root.meshes[0].primitives[0].mode != GLTF_TRIANGLES
        || root.meshes[0].primitives[0].material != 0
        || root.materials.len() != 1
        || root.materials[0].name != "ORIGAMI2 Paper"
        || root.materials[0].pbr.base_color_factor
            != mesh
                .base_color_rgba
                .map(|channel| f32::from(channel) / 255.0)
        || root.materials[0].pbr.metallic_factor.to_bits() != 0.0_f32.to_bits()
        || root.materials[0].pbr.roughness_factor.to_bits() != 1.0_f32.to_bits()
        || root.materials[0]
            .pbr
            .base_color_texture
            .as_ref()
            .map(|v| (v.index, v.tex_coord))
            != mesh.base_color_texture.as_ref().map(|_| (0, 0))
        || !root.materials[0].double_sided
        || root.buffers.len() != 1
        || root.buffers[0].byte_length != binary_length
        || root.buffer_views.len()
            != 3 + usize::from(!mesh.vertex_colors_rgba.is_empty())
                + usize::from(mesh.base_color_texture.is_some()) * 2
        || root.accessors.len()
            != 3 + usize::from(!mesh.vertex_colors_rgba.is_empty())
                + usize::from(mesh.base_color_texture.is_some())
        || root.images.len() != usize::from(mesh.base_color_texture.is_some())
        || root.textures.len() != usize::from(mesh.base_color_texture.is_some())
    {
        return false;
    }
    let position_bytes = match mesh.positions_mm.len().checked_mul(12) {
        Some(value) => value,
        None => return false,
    };
    let normal_offset = position_bytes;
    let color_offset = match normal_offset.checked_add(position_bytes) {
        Some(value) => value,
        None => return false,
    };
    let color_bytes = match mesh.vertex_colors_rgba.len().checked_mul(4) {
        Some(value) => value,
        None => return false,
    };
    let index_offset = match color_offset.checked_add(color_bytes) {
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
    let mut expected_views = vec![
        (0, 0, position_bytes, Some(GLTF_ARRAY_BUFFER)),
        (0, normal_offset, position_bytes, Some(GLTF_ARRAY_BUFFER)),
        (
            0,
            index_offset,
            index_bytes,
            Some(GLTF_ELEMENT_ARRAY_BUFFER),
        ),
    ];
    if !mesh.vertex_colors_rgba.is_empty() {
        expected_views.push((0, color_offset, color_bytes, Some(GLTF_ARRAY_BUFFER)));
    }
    let uv_offset = index_offset + index_bytes;
    if let Some(texture) = &mesh.base_color_texture {
        expected_views.push((
            0,
            uv_offset,
            texture.tex_coords.len() * 8,
            Some(GLTF_ARRAY_BUFFER),
        ));
        expected_views.push((
            0,
            uv_offset + texture.tex_coords.len() * 8,
            texture.bytes.len(),
            None,
        ));
    }
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
    let mut expected_accessors = vec![
        (0, GLTF_FLOAT, mesh.positions_mm.len(), "VEC3", None),
        (1, GLTF_FLOAT, mesh.normals.len(), "VEC3", None),
        (2, GLTF_UNSIGNED_INT, index_count, "SCALAR", None),
    ];
    if !mesh.vertex_colors_rgba.is_empty() {
        expected_accessors.push((
            3,
            GLTF_UNSIGNED_BYTE,
            mesh.vertex_colors_rgba.len(),
            "VEC4",
            Some(true),
        ));
    }
    if let Some(texture) = &mesh.base_color_texture {
        expected_accessors.push((
            if mesh.vertex_colors_rgba.is_empty() {
                3
            } else {
                4
            },
            GLTF_FLOAT,
            texture.tex_coords.len(),
            "VEC2",
            None,
        ));
        let image_view = root.buffer_views.len() - 1;
        if root.images[0].buffer_view != image_view as u32
            || root.images[0].mime_type != texture.media_type.as_str()
            || root.textures[0].source != 0
        {
            return false;
        }
    }
    root.accessors
        .iter()
        .zip(expected_accessors)
        .all(|(actual, expected)| {
            actual.buffer_view == expected.0
                && actual.byte_offset == 0
                && actual.component_type == expected.1
                && actual.count == expected.2
                && actual.accessor_type == expected.3
                && actual.normalized == expected.4
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
    for expected in &mesh.vertex_colors_rgba {
        let Some(actual) = binary.get(cursor..cursor + 4) else {
            return false;
        };
        if actual != expected {
            return false;
        }
        cursor += 4;
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
    if let Some(texture) = &mesh.base_color_texture {
        for expected in texture.tex_coords.iter().flatten() {
            let Some(actual) = read_f32_le(binary, &mut cursor) else {
                return false;
            };
            if actual.to_bits() != expected.to_bits() {
                return false;
            }
        }
        let Some(actual) = binary.get(cursor..cursor + texture.bytes.len()) else {
            return false;
        };
        if actual != texture.bytes {
            return false;
        }
        cursor += texture.bytes.len();
        if binary[cursor..].iter().any(|byte| *byte != 0) {
            return false;
        }
        cursor = binary.len();
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
    root.accessors[0]
        .min
        .as_deref()
        .is_some_and(|value| check_number_vec_f32(value, position_min))
        && root.accessors[0]
            .max
            .as_deref()
            .is_some_and(|value| check_number_vec_f32(value, position_max))
        && root.accessors[1]
            .min
            .as_deref()
            .is_some_and(|value| check_number_vec_f32(value, normal_min))
        && root.accessors[1]
            .max
            .as_deref()
            .is_some_and(|value| check_number_vec_f32(value, normal_max))
        && root.accessors[2]
            .min
            .as_deref()
            .is_some_and(|value| check_number_vec_u32(value, [index_min]))
        && root.accessors[2]
            .max
            .as_deref()
            .is_some_and(|value| check_number_vec_u32(value, [index_max]))
        && root
            .accessors
            .iter()
            .skip(3)
            .all(|accessor| accessor.min.is_none() && accessor.max.is_none())
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

    #[test]
    fn glb_embeds_bounded_png_texture_and_uvs_for_independent_reader() {
        // The image reader is deliberately not invoked by this interchange
        // test; these bytes are a complete 1x1 RGBA PNG kept in source only.
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ];
        let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let document = sample_document().with_base_color_texture(EmbeddedBaseColorTextureV1 {
            media_type: EmbeddedTextureMediaTypeV1::Png,
            bytes: png.clone(),
            tex_coords: uvs.clone(),
        });
        let mesh = validate_indexed_triangle_mesh(&document).unwrap();
        let artifact = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).unwrap();
        let gltf = gltf::Gltf::from_slice(&artifact.bytes).expect("independent glTF reader");
        let blob = gltf.blob.as_deref().unwrap();
        let primitive = gltf.meshes().next().unwrap().primitives().next().unwrap();
        let read_uvs: Vec<_> = primitive
            .reader(|_| Some(blob))
            .read_tex_coords(0)
            .unwrap()
            .into_f32()
            .collect();
        assert_eq!(read_uvs, uvs);
        let material = primitive.material();
        assert_eq!(
            material
                .pbr_metallic_roughness()
                .base_color_texture()
                .unwrap()
                .texture()
                .index(),
            0
        );
        let image = gltf.images().next().unwrap();
        let gltf::image::Source::View { view, mime_type } = image.source() else {
            panic!("embedded image required")
        };
        assert_eq!(mime_type, "image/png");
        assert_eq!(&blob[view.offset()..view.offset() + view.length()], png);
    }

    #[test]
    fn dual_sided_glb_has_independent_front_and_back_primitives_materials_and_images() {
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ];
        let front_uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let back_uvs = vec![[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let document = sample_document().with_base_color_texture(EmbeddedBaseColorTextureV1 {
            media_type: EmbeddedTextureMediaTypeV1::Png,
            bytes: png.clone(),
            tex_coords: front_uvs.clone(),
        });
        let mesh = validate_indexed_triangle_mesh(&document).unwrap();
        let back = EmbeddedBaseColorTextureV1 {
            media_type: EmbeddedTextureMediaTypeV1::Png,
            bytes: png.clone(),
            tex_coords: back_uvs.clone(),
        };
        let artifact =
            export_dual_sided_triangle_mesh_glb(&mesh, back.clone(), [20, 30, 40, 255]).unwrap();
        let gltf = gltf::Gltf::from_slice(&artifact.bytes).expect("independent glTF reader");
        let blob = gltf.blob.as_deref().unwrap();
        let primitives = gltf
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .collect::<Vec<_>>();
        assert_eq!(primitives.len(), 2);
        assert_eq!(gltf.materials().count(), 2);
        assert_eq!(gltf.images().count(), 2);
        assert_eq!(gltf.textures().count(), 2);
        let front_indices = primitives[0]
            .reader(|_| Some(blob))
            .read_indices()
            .unwrap()
            .into_u32()
            .collect::<Vec<_>>();
        let back_indices = primitives[1]
            .reader(|_| Some(blob))
            .read_indices()
            .unwrap()
            .into_u32()
            .collect::<Vec<_>>();
        assert_eq!(front_indices, vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(back_indices, vec![0, 2, 1, 0, 3, 2]);
        let front_normals = primitives[0]
            .reader(|_| Some(blob))
            .read_normals()
            .unwrap()
            .collect::<Vec<_>>();
        let back_normals = primitives[1]
            .reader(|_| Some(blob))
            .read_normals()
            .unwrap()
            .collect::<Vec<_>>();
        assert!(front_normals.iter().all(|normal| normal[2] == 1.0));
        assert!(back_normals.iter().all(|normal| normal[2] == -1.0));
        let read_back_uvs = primitives[1]
            .reader(|_| Some(blob))
            .read_tex_coords(0)
            .unwrap()
            .into_f32()
            .collect::<Vec<_>>();
        assert_eq!(read_back_uvs, back_uvs);
        assert_eq!(primitives[0].material().index(), Some(0));
        assert_eq!(primitives[1].material().index(), Some(1));
        for image in gltf.images() {
            let gltf::image::Source::View { view, mime_type } = image.source() else {
                panic!("embedded image required")
            };
            assert_eq!(mime_type, "image/png");
            assert_eq!(&blob[view.offset()..view.offset() + view.length()], png);
        }
        assert_eq!(artifact.triangle_count, 4);

        let mut invalid = back.clone();
        invalid.tex_coords.pop();
        assert!(matches!(
            export_dual_sided_triangle_mesh_glb(&mesh, invalid, [0; 4]),
            Err(StaticMeshExportError::TextureCoordinateCountMismatch { .. })
        ));
        let exact = StaticMeshExportLimits {
            max_output_bytes: artifact.bytes.len(),
            ..StaticMeshExportLimits::default()
        };
        assert!(
            export_dual_sided_triangle_mesh_glb_with_limits(
                &mesh,
                back.clone(),
                [20, 30, 40, 255],
                exact,
            )
            .is_ok()
        );
        let one_short = StaticMeshExportLimits {
            max_output_bytes: artifact.bytes.len() - 1,
            ..StaticMeshExportLimits::default()
        };
        assert!(matches!(
            export_dual_sided_triangle_mesh_glb_with_limits(
                &mesh,
                back,
                [20, 30, 40, 255],
                one_short
            ),
            Err(StaticMeshExportError::OutputTooLarge { .. })
        ));
    }

    #[test]
    fn closed_solid_glb_regions_are_complete_and_side_wall_is_untextured() {
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207,
            192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ];
        let uvs = vec![[0.0, 0.0]; 6];
        let document = IndexedTriangleMeshV1::new(
            "closed prism",
            vec![
                [0.0, 0.0, 1.0],
                [10.0, 0.0, 1.0],
                [0.0, 10.0, 1.0],
                [0.0, 0.0, -1.0],
                [10.0, 0.0, -1.0],
                [0.0, 10.0, -1.0],
            ],
            vec![[0.0, 0.0, 1.0]; 6],
            vec![
                [0, 1, 2],
                [3, 5, 4],
                [0, 3, 4],
                [0, 4, 1],
                [1, 4, 5],
                [1, 5, 2],
                [2, 5, 3],
                [2, 3, 0],
            ],
        )
        .with_base_color_texture(EmbeddedBaseColorTextureV1 {
            media_type: EmbeddedTextureMediaTypeV1::Png,
            bytes: png.clone(),
            tex_coords: uvs.clone(),
        });
        let mesh = validate_indexed_triangle_mesh(&document).unwrap();
        let regions = vec![
            ClosedSolidTriangleRegionV1::FrontCap,
            ClosedSolidTriangleRegionV1::BackCap,
            ClosedSolidTriangleRegionV1::SideWall,
            ClosedSolidTriangleRegionV1::SideWall,
            ClosedSolidTriangleRegionV1::SideWall,
            ClosedSolidTriangleRegionV1::SideWall,
            ClosedSolidTriangleRegionV1::SideWall,
            ClosedSolidTriangleRegionV1::SideWall,
        ];
        let artifact = export_regioned_closed_solid_triangle_mesh_glb(
            &mesh,
            &regions,
            EmbeddedBaseColorTextureV1 {
                media_type: EmbeddedTextureMediaTypeV1::Png,
                bytes: png.clone(),
                tex_coords: uvs,
            },
            [10, 20, 30, 255],
            [80, 70, 60, 255],
        )
        .unwrap();
        let gltf = gltf::Gltf::from_slice(&artifact.bytes).unwrap();
        let blob = gltf.blob.as_deref().unwrap();
        let primitives = gltf
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .collect::<Vec<_>>();
        assert_eq!(primitives.len(), 3);
        assert_eq!(gltf.materials().count(), 3);
        assert_eq!(gltf.images().count(), 2);
        assert_eq!(gltf.textures().count(), 2);
        assert_eq!(
            primitives
                .iter()
                .map(|primitive| primitive
                    .reader(|_| Some(blob))
                    .read_indices()
                    .unwrap()
                    .into_u32()
                    .count())
                .collect::<Vec<_>>(),
            vec![3, 3, 18]
        );
        assert!(
            primitives[0]
                .reader(|_| Some(blob))
                .read_tex_coords(0)
                .is_some()
        );
        assert!(
            primitives[1]
                .reader(|_| Some(blob))
                .read_tex_coords(0)
                .is_some()
        );
        assert!(
            primitives[2]
                .reader(|_| Some(blob))
                .read_tex_coords(0)
                .is_none()
        );
        assert!(
            primitives[2]
                .material()
                .pbr_metallic_roughness()
                .base_color_texture()
                .is_none()
        );
        for image in gltf.images() {
            let gltf::image::Source::View { view, .. } = image.source() else {
                panic!("embedded image")
            };
            assert_eq!(&blob[view.offset()..view.offset() + view.length()], png);
        }

        assert!(matches!(
            export_regioned_closed_solid_triangle_mesh_glb(
                &mesh,
                &regions[..regions.len() - 1],
                EmbeddedBaseColorTextureV1 {
                    media_type: EmbeddedTextureMediaTypeV1::Png,
                    bytes: png,
                    tex_coords: vec![[0.0, 0.0]; 6],
                },
                [0; 4],
                [0; 4],
            ),
            Err(StaticMeshExportError::TriangleRegionCountMismatch)
        ));
    }

    #[test]
    fn texture_admission_rejects_bad_payload_uvs_and_resource_excess() {
        let mut document = sample_document();
        document.base_color_texture = Some(EmbeddedBaseColorTextureV1 {
            media_type: EmbeddedTextureMediaTypeV1::Jpeg,
            bytes: vec![0xff, 0xd8, 0xff, 0xd9],
            tex_coords: vec![[0.0, 0.0]; 3],
        });
        assert!(matches!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::TextureCoordinateCountMismatch { .. })
        ));
        document.base_color_texture.as_mut().unwrap().tex_coords = vec![[0.0, 0.0]; 4];
        document.base_color_texture.as_mut().unwrap().bytes = vec![0; 4];
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::InvalidTexturePayload)
        );
        document.base_color_texture.as_mut().unwrap().bytes =
            vec![0; MAX_STATIC_MESH_TEXTURE_BYTES + 1];
        assert!(matches!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::TextureTooLarge { .. })
        ));
    }

    fn sample_mesh() -> ValidatedIndexedTriangleMesh {
        validate_indexed_triangle_mesh(&sample_document()).expect("sample mesh")
    }

    #[test]
    fn glb_generation_provenance_extension_round_trips_and_legacy_is_absent() {
        let mesh = sample_mesh();
        let provenance = ori_domain::BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256: [0x17; 32],
            fold_path_certificate_sha256: Some([0x71; 32]),
            confidence_score: 90,
            confidence_reasons: vec!["bounded_native_fold_path_v2".to_owned()],
            explicit_override: false,
            source_asset_fingerprint: "asset:glb-extension".to_owned(),
            semantic_landmark_provenance: None,
        };
        let artifact = export_static_triangle_mesh_glb_with_provenance(&mesh, &provenance)
            .expect("GLB provenance export");
        assert_eq!(
            read_glb_generation_provenance(&artifact.bytes).expect("GLB provenance read"),
            Some(provenance)
        );
        let json_length = read_u32_le_at(&artifact.bytes, 12).unwrap() as usize;
        let json_end = 20 + json_length;
        let binary_length = read_u32_le_at(&artifact.bytes, json_end).unwrap() as usize;
        let mut root: serde_json::Value =
            serde_json::from_slice(&artifact.bytes[20..json_end]).unwrap();
        root["extensions"][ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1]["provenance"]["confidence_score"] =
            serde_json::json!(89);
        let tampered = encode_glb_root_and_binary(
            &root,
            &artifact.bytes[json_end + 8..json_end + 8 + binary_length],
            MAX_STATIC_MESH_EXPORT_BYTES,
        )
        .unwrap();
        assert!(read_glb_generation_provenance(&tampered).is_err());
        let legacy =
            export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).expect("legacy GLB");
        assert_eq!(read_glb_generation_provenance(&legacy.bytes).unwrap(), None);
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
    fn vertex_colors_are_optional_but_must_cover_every_vertex() {
        let legacy = serde_json::json!({
            "schema_version": 1,
            "name": "mesh",
            "positions_mm": [[0,0,0],[1,0,0],[0,1,0]],
            "normals": [[0,0,1],[0,0,1],[0,0,1]],
            "triangles": [[0,1,2]]
        });
        let legacy: IndexedTriangleMeshV1 =
            serde_json::from_value(legacy).expect("legacy mesh without colors");
        assert!(legacy.vertex_colors_rgba.is_empty());

        let mut document = sample_document();
        document.vertex_colors_rgba = vec![[255, 0, 0, 255]; 3];
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::VertexColorCountMismatch {
                actual: 3,
                expected: 4,
            })
        );

        document.vertex_colors_rgba.push([0, 0, 255, 255]);
        let validated = validate_indexed_triangle_mesh(&document).expect("colored mesh");
        assert_eq!(validated.vertex_colors_rgba(), document.vertex_colors_rgba);
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
            "\"materials\"",
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
        assert!(json.contains("\"material\":0"));

        let colored = validate_indexed_triangle_mesh(
            &sample_document().with_base_color_rgba([12, 34, 56, 255]),
        )
        .expect("colored mesh");
        let colored_glb =
            export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &colored).expect("GLB");
        let colored_json_length =
            usize::try_from(read_u32_le_at(&colored_glb.bytes, 12).unwrap()).unwrap();
        let colored_json =
            std::str::from_utf8(&colored_glb.bytes[20..20 + colored_json_length]).unwrap();
        assert!(
            colored_json.contains("\"baseColorFactor\":[0.047058824,0.13333334,0.21960784,1.0]")
        );

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
    fn glb_preserves_normalized_rgba_vertex_colors() {
        let colors = vec![
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 255, 128],
        ];
        let mesh = validate_indexed_triangle_mesh(
            &sample_document().with_vertex_colors_rgba(colors.clone()),
        )
        .expect("colored mesh");
        let artifact =
            export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).expect("GLB");
        let json_length = usize::try_from(read_u32_le_at(&artifact.bytes, 12).unwrap()).unwrap();
        let json = std::str::from_utf8(&artifact.bytes[20..20 + json_length]).unwrap();
        assert!(json.contains("\"COLOR_0\":3"));
        assert!(
            json.contains(
                "\"componentType\":5121,\"count\":4,\"type\":\"VEC4\",\"normalized\":true"
            )
        );
        assert!(verify_glb(&artifact.bytes, &mesh, artifact.bytes.len()));

        let binary_header = 20 + json_length;
        let binary_start = binary_header + GLB_CHUNK_HEADER_BYTES;
        let color_start = binary_start + mesh.positions_mm.len() * 24;
        assert_eq!(
            &artifact.bytes[color_start..color_start + colors.len() * 4],
            colors.as_flattened()
        );
        let mut changed = artifact.bytes;
        changed[color_start] = 0;
        assert!(!verify_glb(&changed, &mesh, changed.len()));
    }

    #[test]
    fn ecosystem_readers_accept_all_three_interchange_formats() {
        use std::io::Cursor;

        let colors = vec![
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 255, 128],
        ];
        let mesh = validate_indexed_triangle_mesh(
            &sample_document().with_vertex_colors_rgba(colors.clone()),
        )
        .expect("colored mesh");

        let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
            .expect("OBJ")
            .bytes;
        let (models, materials) = tobj::load_obj_buf(
            &mut Cursor::new(obj),
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..tobj::LoadOptions::default()
            },
            |_| Ok((Vec::new(), Default::default())),
        )
        .expect("tobj accepts OBJ");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].mesh.positions.len(), mesh.positions_mm.len() * 3);
        assert_eq!(models[0].mesh.indices.len(), mesh.triangles.len() * 3);
        assert!(materials.expect("material result").is_empty());

        let stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh)
            .expect("STL")
            .bytes;
        let parsed_stl = stl_io::read_stl(&mut Cursor::new(stl)).expect("stl_io accepts STL");
        assert_eq!(parsed_stl.faces.len(), mesh.triangles.len());

        let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
            .expect("GLB")
            .bytes;
        let parsed_glb = gltf::Gltf::from_slice(&glb).expect("gltf validator accepts GLB");
        assert_eq!(parsed_glb.scenes().count(), 1);
        let primitive = parsed_glb
            .meshes()
            .next()
            .and_then(|mesh| mesh.primitives().next())
            .expect("one primitive");
        let reader = primitive.reader(|_| parsed_glb.blob.as_deref());
        assert_eq!(
            reader.read_positions().expect("positions").count(),
            mesh.positions_mm.len()
        );
        assert_eq!(
            reader.read_indices().expect("indices").into_u32().count(),
            mesh.triangles.len() * 3
        );
        let parsed_colors: Vec<_> = reader
            .read_colors(0)
            .expect("vertex colors")
            .into_rgba_u8()
            .collect();
        assert_eq!(parsed_colors, colors);
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
