//! Strict, bounded FOLD 1.2 multi-frame 3D preview.
//!
//! This is intentionally separate from the established 2D importer. A frame
//! selection is immutable interchange evidence only; it does not authorize a
//! project replacement, native applied pose, or instruction-timeline write.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{FoldImportLimits, fold::MAX_SUPPORTED_FOLD_SPEC};

pub const MAX_FOLD_3D_FRAMES_V1: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fold3dFramePreviewV1 {
    index: usize,
    parent: Option<usize>,
    inherits: bool,
    vertex_count: usize,
}

impl Fold3dFramePreviewV1 {
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }
    #[must_use]
    pub const fn parent(&self) -> Option<usize> {
        self.parent
    }
    #[must_use]
    pub const fn inherits(&self) -> bool {
        self.inherits
    }
    #[must_use]
    pub const fn vertex_count(&self) -> usize {
        self.vertex_count
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fold3dFramesPreviewV1 {
    source_sha256: [u8; 32],
    frames: Vec<Fold3dFramePreviewV1>,
    resolved_vertices: Vec<Vec<[f64; 3]>>,
}

impl Fold3dFramesPreviewV1 {
    #[must_use]
    pub fn frames(&self) -> &[Fold3dFramePreviewV1] {
        &self.frames
    }

    pub fn select_frame(
        &self,
        index: usize,
    ) -> Result<Fold3dFrameSelectionV1, Fold3dFramesImportErrorV1> {
        let vertices = self
            .resolved_vertices
            .get(index)
            .cloned()
            .ok_or(Fold3dFramesImportErrorV1::FrameSelectionOutOfRange)?;
        Ok(Fold3dFrameSelectionV1 {
            source_sha256: self.source_sha256,
            frame_index: index,
            vertices,
            authorizes_project_mutation: false,
            authorizes_applied_pose: false,
            authorizes_instruction_timeline: false,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fold3dFrameSelectionV1 {
    source_sha256: [u8; 32],
    frame_index: usize,
    vertices: Vec<[f64; 3]>,
    authorizes_project_mutation: bool,
    authorizes_applied_pose: bool,
    authorizes_instruction_timeline: bool,
}

impl Fold3dFrameSelectionV1 {
    #[must_use]
    pub const fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    #[must_use]
    pub const fn frame_index(&self) -> usize {
        self.frame_index
    }
    #[must_use]
    pub fn vertices(&self) -> &[[f64; 3]] {
        &self.vertices
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_applied_pose(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_instruction_timeline(&self) -> bool {
        false
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Fold3dFramesImportErrorV1 {
    #[error("FOLD file exceeds the configured byte limit")]
    FileTooLarge,
    #[error("FOLD multi-frame JSON is malformed")]
    Malformed,
    #[error("FOLD specification version is unsupported")]
    UnsupportedSpec,
    #[error("FOLD frame count exceeds the supported bound")]
    TooManyFrames,
    #[error("FOLD frame parent graph is invalid or cyclic")]
    InvalidFrameParent,
    #[error("FOLD inherited frame geometry is unavailable")]
    MissingInheritedGeometry,
    #[error("FOLD 3D coordinate count exceeds the configured bound")]
    TooManyVertices,
    #[error("FOLD 3D coordinates must be finite triples")]
    InvalidCoordinates,
    #[error("selected FOLD frame is out of range")]
    FrameSelectionOutOfRange,
}

#[derive(Deserialize)]
struct RawDocument {
    file_spec: Option<f64>,
    #[serde(default)]
    vertices_coords: Option<Vec<Vec<f64>>>,
    file_frames: Vec<RawFrame>,
}

#[derive(Deserialize)]
struct RawFrame {
    #[serde(default)]
    frame_parent: Option<usize>,
    #[serde(default)]
    frame_inherit: bool,
    #[serde(default)]
    vertices_coords: Option<Vec<Vec<f64>>>,
}

pub fn read_fold_3d_frames_preview_v1(
    bytes: &[u8],
    limits: FoldImportLimits,
) -> Result<Fold3dFramesPreviewV1, Fold3dFramesImportErrorV1> {
    if bytes.len() > limits.max_file_bytes {
        return Err(Fold3dFramesImportErrorV1::FileTooLarge);
    }
    let raw: RawDocument =
        serde_json::from_slice(bytes).map_err(|_| Fold3dFramesImportErrorV1::Malformed)?;
    if raw
        .file_spec
        .is_some_and(|spec| !spec.is_finite() || spec <= 0.0 || spec > MAX_SUPPORTED_FOLD_SPEC)
    {
        return Err(Fold3dFramesImportErrorV1::UnsupportedSpec);
    }
    if raw.file_frames.is_empty() || raw.file_frames.len() > MAX_FOLD_3D_FRAMES_V1 {
        return Err(Fold3dFramesImportErrorV1::TooManyFrames);
    }
    if raw.file_frames.iter().enumerate().any(|(index, frame)| {
        frame
            .frame_parent
            .is_some_and(|parent| parent >= raw.file_frames.len() || parent == index)
    }) {
        return Err(Fold3dFramesImportErrorV1::InvalidFrameParent);
    }
    let root = raw
        .vertices_coords
        .map(|coords| coordinates(coords, limits.max_vertices))
        .transpose()?;
    let mut resolved = vec![None; raw.file_frames.len()];
    let mut visiting = HashSet::new();
    for index in 0..raw.file_frames.len() {
        resolve(
            index,
            &raw.file_frames,
            root.as_ref(),
            limits.max_vertices,
            &mut resolved,
            &mut visiting,
        )?;
    }
    let resolved_vertices = resolved.into_iter().map(Option::unwrap).collect::<Vec<_>>();
    let frames = raw
        .file_frames
        .iter()
        .enumerate()
        .map(|(index, frame)| Fold3dFramePreviewV1 {
            index,
            parent: frame.frame_parent,
            inherits: frame.frame_inherit,
            vertex_count: resolved_vertices[index].len(),
        })
        .collect();
    Ok(Fold3dFramesPreviewV1 {
        source_sha256: Sha256::digest(bytes).into(),
        frames,
        resolved_vertices,
    })
}

fn resolve(
    index: usize,
    frames: &[RawFrame],
    root: Option<&Vec<[f64; 3]>>,
    maximum: usize,
    resolved: &mut [Option<Vec<[f64; 3]>>],
    visiting: &mut HashSet<usize>,
) -> Result<(), Fold3dFramesImportErrorV1> {
    if resolved[index].is_some() {
        return Ok(());
    }
    if !visiting.insert(index) {
        return Err(Fold3dFramesImportErrorV1::InvalidFrameParent);
    }
    let frame = &frames[index];
    let own = frame
        .vertices_coords
        .clone()
        .map(|value| coordinates(value, maximum))
        .transpose()?;
    let inherited = if frame.frame_inherit {
        match frame.frame_parent {
            Some(parent) if parent < frames.len() && parent != index => {
                resolve(parent, frames, root, maximum, resolved, visiting)?;
                resolved[parent].clone()
            }
            Some(_) => return Err(Fold3dFramesImportErrorV1::InvalidFrameParent),
            None => root.cloned(),
        }
    } else {
        None
    };
    let value = own
        .or(inherited)
        .ok_or(Fold3dFramesImportErrorV1::MissingInheritedGeometry)?;
    resolved[index] = Some(value);
    visiting.remove(&index);
    Ok(())
}

fn coordinates(
    values: Vec<Vec<f64>>,
    maximum: usize,
) -> Result<Vec<[f64; 3]>, Fold3dFramesImportErrorV1> {
    if values.len() > maximum {
        return Err(Fold3dFramesImportErrorV1::TooManyVertices);
    }
    values
        .into_iter()
        .map(|value| {
            let [x, y, z]: [f64; 3] = value
                .try_into()
                .map_err(|_| Fold3dFramesImportErrorV1::InvalidCoordinates)?;
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                return Err(Fold3dFramesImportErrorV1::InvalidCoordinates);
            }
            Ok([canonical(x), canonical(y), canonical(z)])
        })
        .collect()
}

fn canonical(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_parent_inheritance_and_authenticates_selection() {
        let bytes = br#"{
          "file_spec":1.2,
          "vertices_coords":[[0,0,0],[1,0,0],[0,1,0]],
          "file_frames":[
            {"frame_inherit":true},
            {"frame_parent":0,"frame_inherit":true},
            {"frame_parent":1,"frame_inherit":true,"vertices_coords":[[0,0,0],[1,0,1],[0,1,0]]}
          ]
        }"#;
        let preview = read_fold_3d_frames_preview_v1(bytes, FoldImportLimits::default()).unwrap();
        assert_eq!(preview.frames().len(), 3);
        assert_eq!(
            preview.select_frame(1).unwrap().vertices()[1],
            [1.0, 0.0, 0.0]
        );
        let selected = preview.select_frame(2).unwrap();
        assert_eq!(selected.vertices()[1], [1.0, 0.0, 1.0]);
        assert!(!selected.authorizes_project_mutation());
        assert!(!selected.authorizes_applied_pose());
        assert!(!selected.authorizes_instruction_timeline());
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        assert_eq!(selected.source_sha256(), digest);
    }

    #[test]
    fn cycles_malformed_coordinates_and_bounds_fail_closed() {
        let cycle = br#"{"file_frames":[
          {"frame_parent":1,"frame_inherit":true},
          {"frame_parent":0,"frame_inherit":true}
        ]}"#;
        assert_eq!(
            read_fold_3d_frames_preview_v1(cycle, FoldImportLimits::default()),
            Err(Fold3dFramesImportErrorV1::InvalidFrameParent)
        );
        let malformed = br#"{"file_frames":[{"vertices_coords":[[0,0],[1,0,0]]}]}"#;
        assert_eq!(
            read_fold_3d_frames_preview_v1(malformed, FoldImportLimits::default()),
            Err(Fold3dFramesImportErrorV1::InvalidCoordinates)
        );
        let mut limits = FoldImportLimits::default();
        limits.max_vertices = 1;
        let bounded = br#"{"file_frames":[{"vertices_coords":[[0,0,0],[1,0,0]]}]}"#;
        assert_eq!(
            read_fold_3d_frames_preview_v1(bounded, limits),
            Err(Fold3dFramesImportErrorV1::TooManyVertices)
        );
    }
}
