//! Bounded glTF 2.0 animation export for meshes with invariant topology.
//!
//! Every keyframe is admitted through the static-mesh contract. Animation is
//! encoded as STEP-interpolated morph targets, which preserves every supplied
//! frame exactly and remains readable by ordinary glTF tooling.

use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::{
    IndexedTriangleMeshV1, MAX_STATIC_MESH_EXPORT_BYTES, StaticMeshExportError,
    StaticMeshExportLimits, validate_indexed_triangle_mesh_with_limits,
};

pub const INDEXED_TRIANGLE_MESH_ANIMATION_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_MESH_ANIMATION_FRAMES: usize = 256;
pub const MAX_MESH_ANIMATION_DURATION_SECONDS: f32 = 86_400.0;

const GLB_JSON_CHUNK_TYPE: u32 = 0x4e4f_534a;
const GLB_BIN_CHUNK_TYPE: u32 = 0x004e_4942;
const NODE_MATRIX: [f32; 16] = [
    -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedTriangleMeshAnimationV1 {
    pub schema_version: u32,
    pub times_seconds: Vec<f32>,
    pub frames: Vec<IndexedTriangleMeshV1>,
}

impl IndexedTriangleMeshAnimationV1 {
    #[must_use]
    pub fn new(times_seconds: Vec<f32>, frames: Vec<IndexedTriangleMeshV1>) -> Self {
        Self {
            schema_version: INDEXED_TRIANGLE_MESH_ANIMATION_SCHEMA_VERSION_V1,
            times_seconds,
            frames,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshAnimationExportArtifact {
    pub media_type: &'static str,
    pub file_extension: &'static str,
    pub bytes: Vec<u8>,
    pub frame_count: usize,
    pub vertex_count: usize,
    pub triangle_count: usize,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum MeshAnimationExportError {
    #[error("unsupported mesh-animation schema version {actual}")]
    UnsupportedSchemaVersion { actual: u32 },
    #[error("animation requires between 2 and {maximum} frames")]
    InvalidFrameCount { maximum: usize },
    #[error("animation time count must equal frame count")]
    TimeCountMismatch,
    #[error("animation times must be finite, non-negative, strictly increasing, and bounded")]
    InvalidTimes,
    #[error("frame {frame} failed static mesh admission: {source}")]
    InvalidFrame {
        frame: usize,
        source: StaticMeshExportError,
    },
    #[error("frame {frame} topology or vertex identity differs from frame zero")]
    InconsistentTopology { frame: usize },
    #[error("animated GLB exceeds the {maximum}-byte output limit")]
    OutputTooLarge { maximum: usize },
    #[error("animated GLB size arithmetic overflowed")]
    SizeOverflow,
}

pub fn export_animated_triangle_mesh_glb(
    document: &IndexedTriangleMeshAnimationV1,
) -> Result<MeshAnimationExportArtifact, MeshAnimationExportError> {
    export_animated_triangle_mesh_glb_with_limits(document, StaticMeshExportLimits::default())
}

pub fn export_animated_triangle_mesh_glb_with_limits(
    document: &IndexedTriangleMeshAnimationV1,
    limits: StaticMeshExportLimits,
) -> Result<MeshAnimationExportArtifact, MeshAnimationExportError> {
    if document.schema_version != INDEXED_TRIANGLE_MESH_ANIMATION_SCHEMA_VERSION_V1 {
        return Err(MeshAnimationExportError::UnsupportedSchemaVersion {
            actual: document.schema_version,
        });
    }
    if !(2..=MAX_MESH_ANIMATION_FRAMES).contains(&document.frames.len()) {
        return Err(MeshAnimationExportError::InvalidFrameCount {
            maximum: MAX_MESH_ANIMATION_FRAMES,
        });
    }
    if document.times_seconds.len() != document.frames.len() {
        return Err(MeshAnimationExportError::TimeCountMismatch);
    }
    let valid_times = document
        .times_seconds
        .iter()
        .enumerate()
        .all(|(index, time)| {
            time.is_finite()
                && *time >= 0.0
                && *time <= MAX_MESH_ANIMATION_DURATION_SECONDS
                && (index == 0 || *time > document.times_seconds[index - 1])
        });
    if !valid_times {
        return Err(MeshAnimationExportError::InvalidTimes);
    }

    let frames = document
        .frames
        .iter()
        .enumerate()
        .map(|(frame, mesh)| {
            validate_indexed_triangle_mesh_with_limits(mesh, limits)
                .map_err(|source| MeshAnimationExportError::InvalidFrame { frame, source })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let base = &frames[0];
    for (frame, candidate) in frames.iter().enumerate().skip(1) {
        if candidate.name() != base.name()
            || candidate.triangles() != base.triangles()
            || candidate.positions_mm().len() != base.positions_mm().len()
            || candidate.vertex_colors_rgba() != base.vertex_colors_rgba()
            || candidate.base_color_rgba() != base.base_color_rgba()
        {
            return Err(MeshAnimationExportError::InconsistentTopology { frame });
        }
    }

    let bytes = serialize(
        document,
        &frames,
        limits.max_output_bytes.min(MAX_STATIC_MESH_EXPORT_BYTES),
    )?;
    Ok(MeshAnimationExportArtifact {
        media_type: "model/gltf-binary",
        file_extension: "glb",
        bytes,
        frame_count: frames.len(),
        vertex_count: base.positions_mm().len(),
        triangle_count: base.triangles().len(),
    })
}

fn serialize(
    document: &IndexedTriangleMeshAnimationV1,
    frames: &[crate::ValidatedIndexedTriangleMesh],
    maximum: usize,
) -> Result<Vec<u8>, MeshAnimationExportError> {
    let base = &frames[0];
    let mut binary = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut append = |data: &[u8],
                      target: Option<u32>,
                      component_type: u32,
                      count: usize,
                      kind: &str,
                      min: Option<serde_json::Value>,
                      max: Option<serde_json::Value>| {
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let offset = binary.len();
        binary.extend_from_slice(data);
        let view = views.len();
        let mut view_json = json!({"buffer":0,"byteOffset":offset,"byteLength":data.len()});
        if let Some(target) = target {
            view_json["target"] = json!(target);
        }
        views.push(view_json);
        let mut accessor =
            json!({"bufferView":view,"componentType":component_type,"count":count,"type":kind});
        if let Some(value) = min {
            accessor["min"] = value;
        }
        if let Some(value) = max {
            accessor["max"] = value;
        }
        accessors.push(accessor);
        accessors.len() - 1
    };

    let positions = base
        .positions_mm()
        .iter()
        .flat_map(|p| p.iter())
        .flat_map(|v| ((*v * 0.001) as f32).to_le_bytes())
        .collect::<Vec<_>>();
    let normals = base
        .normals()
        .iter()
        .flat_map(|p| p.iter())
        .flat_map(|v| (*v as f32).to_le_bytes())
        .collect::<Vec<_>>();
    let indices = base
        .triangles()
        .iter()
        .flat_map(|t| t.iter())
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    let min = (0..3)
        .map(|axis| {
            base.positions_mm()
                .iter()
                .map(|p| p[axis] * 0.001)
                .fold(f64::INFINITY, f64::min)
        })
        .collect::<Vec<_>>();
    let max = (0..3)
        .map(|axis| {
            base.positions_mm()
                .iter()
                .map(|p| p[axis] * 0.001)
                .fold(f64::NEG_INFINITY, f64::max)
        })
        .collect::<Vec<_>>();
    let position_accessor = append(
        &positions,
        Some(34_962),
        5_126,
        base.positions_mm().len(),
        "VEC3",
        Some(json!(min)),
        Some(json!(max)),
    );
    let normal_accessor = append(
        &normals,
        Some(34_962),
        5_126,
        base.normals().len(),
        "VEC3",
        None,
        None,
    );
    let index_accessor = append(
        &indices,
        Some(34_963),
        5_125,
        base.triangles().len() * 3,
        "SCALAR",
        Some(json!([0])),
        Some(json!([base.positions_mm().len() - 1])),
    );

    let mut targets = Vec::new();
    for frame in frames {
        let position_delta_min = (0..3)
            .map(|axis| {
                frame
                    .positions_mm()
                    .iter()
                    .zip(base.positions_mm())
                    .map(|(point, base_point)| (point[axis] - base_point[axis]) * 0.001)
                    .fold(f64::INFINITY, f64::min)
            })
            .collect::<Vec<_>>();
        let position_delta_max = (0..3)
            .map(|axis| {
                frame
                    .positions_mm()
                    .iter()
                    .zip(base.positions_mm())
                    .map(|(point, base_point)| (point[axis] - base_point[axis]) * 0.001)
                    .fold(f64::NEG_INFINITY, f64::max)
            })
            .collect::<Vec<_>>();
        let position_delta = frame
            .positions_mm()
            .iter()
            .zip(base.positions_mm())
            .flat_map(|(p, b)| {
                (0..3).flat_map(move |axis| (((p[axis] - b[axis]) * 0.001) as f32).to_le_bytes())
            })
            .collect::<Vec<_>>();
        let normal_delta = frame
            .normals()
            .iter()
            .zip(base.normals())
            .flat_map(|(p, b)| {
                (0..3).flat_map(move |axis| ((p[axis] - b[axis]) as f32).to_le_bytes())
            })
            .collect::<Vec<_>>();
        let p = append(
            &position_delta,
            Some(34_962),
            5_126,
            base.positions_mm().len(),
            "VEC3",
            Some(json!(position_delta_min)),
            Some(json!(position_delta_max)),
        );
        let n = append(
            &normal_delta,
            Some(34_962),
            5_126,
            base.normals().len(),
            "VEC3",
            None,
            None,
        );
        targets.push(json!({"POSITION":p,"NORMAL":n}));
    }
    let time_bytes = document
        .times_seconds
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    let time_accessor = append(
        &time_bytes,
        None,
        5_126,
        frames.len(),
        "SCALAR",
        Some(json!([document.times_seconds[0]])),
        Some(json!([document.times_seconds[frames.len() - 1]])),
    );
    let weights = (0..frames.len())
        .flat_map(|key| {
            (0..frames.len()).map(move |target| if key == target { 1.0_f32 } else { 0.0 })
        })
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let weight_accessor = append(
        &weights,
        None,
        5_126,
        frames.len() * frames.len(),
        "SCALAR",
        None,
        None,
    );
    #[allow(clippy::drop_non_drop)]
    drop(append);
    while binary.len() % 4 != 0 {
        binary.push(0);
    }

    let color = base.base_color_rgba().map(|v| f64::from(v) / 255.0);
    let root = json!({
        "asset":{"version":"2.0","generator":"ORIGAMI2 animated mesh v1"},
        "scene":0,"scenes":[{"nodes":[0]}],
        "nodes":[{"mesh":0,"matrix":NODE_MATRIX}],
        "meshes":[{"name":base.name(),"weights":vec![0.0_f32;frames.len()],
            "primitives":[{"attributes":{"POSITION":position_accessor,"NORMAL":normal_accessor},
                "indices":index_accessor,"material":0,"mode":4,"targets":targets}]}],
        "materials":[{"pbrMetallicRoughness":{"baseColorFactor":color,"metallicFactor":0.0,"roughnessFactor":0.9},"doubleSided":true}],
        "animations":[{"name":"ORIGAMI2 frames","samplers":[{"input":time_accessor,"output":weight_accessor,"interpolation":"STEP"}],
            "channels":[{"sampler":0,"target":{"node":0,"path":"weights"}}]}],
        "buffers":[{"byteLength":binary.len()}],"bufferViews":views,"accessors":accessors
    });
    let mut json_bytes =
        serde_json::to_vec(&root).map_err(|_| MeshAnimationExportError::SizeOverflow)?;
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12usize
        .checked_add(8)
        .and_then(|v| v.checked_add(json_bytes.len()))
        .and_then(|v| v.checked_add(8))
        .and_then(|v| v.checked_add(binary.len()))
        .ok_or(MeshAnimationExportError::SizeOverflow)?;
    if total > maximum || total > u32::MAX as usize {
        return Err(MeshAnimationExportError::OutputTooLarge { maximum });
    }
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(&0x4654_6c67_u32.to_le_bytes());
    output.extend_from_slice(&2_u32.to_le_bytes());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    output.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(&GLB_JSON_CHUNK_TYPE.to_le_bytes());
    output.extend_from_slice(&json_bytes);
    output.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    output.extend_from_slice(&GLB_BIN_CHUNK_TYPE.to_le_bytes());
    output.extend_from_slice(&binary);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(offset: f64) -> IndexedTriangleMeshV1 {
        IndexedTriangleMeshV1::new(
            "fold",
            vec![[0.0, 0.0, offset], [10.0, 0.0, offset], [0.0, 10.0, offset]],
            vec![[0.0, 0.0, 1.0]; 3],
            vec![[0, 1, 2]],
        )
    }

    #[test]
    fn independent_reader_observes_all_exact_step_frames() {
        let input = IndexedTriangleMeshAnimationV1::new(
            vec![0.0, 0.5, 1.0],
            vec![frame(0.0), frame(2.0), frame(5.0)],
        );
        let artifact = export_animated_triangle_mesh_glb(&input).unwrap();
        let gltf = gltf::Gltf::from_slice(&artifact.bytes).expect("independent glTF reader");
        let animation = gltf.animations().next().unwrap();
        let channel = animation.channels().next().unwrap();
        assert_eq!(
            channel.sampler().interpolation(),
            gltf::animation::Interpolation::Step
        );
        let reader = channel.reader(|_| gltf.blob.as_deref());
        assert_eq!(
            reader.read_inputs().unwrap().collect::<Vec<_>>(),
            vec![0.0, 0.5, 1.0]
        );
        let weights = match reader.read_outputs().unwrap() {
            gltf::animation::util::ReadOutputs::MorphTargetWeights(weights) => {
                weights.into_f32().collect::<Vec<_>>()
            }
            _ => panic!("weight channel"),
        };
        assert_eq!(weights, vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        let targets = gltf
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .next()
            .unwrap()
            .reader(|_| gltf.blob.as_deref())
            .read_morph_targets()
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 3);
        let z = targets[2].0.clone().unwrap().collect::<Vec<_>>();
        assert_eq!(z, vec![[0.0, 0.0, 0.005]; 3]);
    }

    #[test]
    fn rejects_unbounded_invalid_and_topology_changing_inputs() {
        let mut bad_time =
            IndexedTriangleMeshAnimationV1::new(vec![0.0, 0.0], vec![frame(0.0), frame(1.0)]);
        assert_eq!(
            export_animated_triangle_mesh_glb(&bad_time).unwrap_err(),
            MeshAnimationExportError::InvalidTimes
        );
        bad_time.times_seconds = vec![0.0, 1.0];
        bad_time.frames[1].triangles = vec![[0, 2, 1]];
        assert_eq!(
            export_animated_triangle_mesh_glb(&bad_time).unwrap_err(),
            MeshAnimationExportError::InconsistentTopology { frame: 1 }
        );
    }
}
