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
    topology_identity_authenticated: bool,
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
    #[must_use]
    pub const fn topology_identity_authenticated(&self) -> bool {
        self.topology_identity_authenticated
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fold3dFramesPreviewV1 {
    source_sha256: [u8; 32],
    frames: Vec<Fold3dFramePreviewV1>,
    resolved_vertices: Vec<Vec<[f64; 3]>>,
    topology_sha256: Vec<Option<[u8; 32]>>,
    topologies: Vec<Option<Topology>>,
    rest_vertices: Option<Vec<[f64; 3]>>,
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
            topology_sha256: self.topology_sha256[index],
            authorizes_project_mutation: false,
            authorizes_applied_pose: false,
            authorizes_instruction_timeline: false,
        })
    }

    pub fn prepare_applied_pose_proposal(
        &self,
        index: usize,
    ) -> Result<Fold3dAppliedPoseProposalV1, Fold3dFramesImportErrorV1> {
        let rest = self
            .rest_vertices
            .as_ref()
            .ok_or(Fold3dFramesImportErrorV1::PoseUnavailable)?;
        let target = self
            .resolved_vertices
            .get(index)
            .ok_or(Fold3dFramesImportErrorV1::FrameSelectionOutOfRange)?;
        let topology = self
            .topologies
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(Fold3dFramesImportErrorV1::PoseUnavailable)?;
        prepare_pose(rest, target, topology, self.source_sha256, index)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fold3dAppliedPoseProposalV1 {
    source_sha256: [u8; 32],
    frame_index: usize,
    face_transforms: Vec<[[f64; 4]; 3]>,
    hinge_angles_degrees: Vec<(usize, f64)>,
    rest_vertices: Vec<[f64; 3]>,
    edges: Vec<[usize; 2]>,
    faces: Vec<Vec<usize>>,
    assignments: Vec<String>,
}

impl Fold3dAppliedPoseProposalV1 {
    #[must_use]
    pub const fn source_sha256(&self) -> [u8; 32] {
        self.source_sha256
    }
    #[must_use]
    pub const fn frame_index(&self) -> usize {
        self.frame_index
    }
    #[must_use]
    pub fn face_transforms(&self) -> &[[[f64; 4]; 3]] {
        &self.face_transforms
    }
    #[must_use]
    pub fn hinge_angles_degrees(&self) -> &[(usize, f64)] {
        &self.hinge_angles_degrees
    }
    #[must_use]
    pub fn rest_vertices(&self) -> &[[f64; 3]] {
        &self.rest_vertices
    }
    #[must_use]
    pub fn edges(&self) -> &[[usize; 2]] {
        &self.edges
    }
    #[must_use]
    pub fn faces(&self) -> &[Vec<usize>] {
        &self.faces
    }
    #[must_use]
    pub fn assignments(&self) -> &[String] {
        &self.assignments
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fold3dFrameSelectionV1 {
    source_sha256: [u8; 32],
    frame_index: usize,
    vertices: Vec<[f64; 3]>,
    topology_sha256: Option<[u8; 32]>,
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
    pub const fn topology_sha256(&self) -> Option<[u8; 32]> {
        self.topology_sha256
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
    #[error("FOLD frame cannot produce the narrow rigid tree pose proposal")]
    PoseUnavailable,
}

#[derive(Deserialize)]
struct RawDocument {
    file_spec: Option<f64>,
    #[serde(default)]
    vertices_coords: Option<Vec<Vec<f64>>>,
    #[serde(default)]
    edges_vertices: Option<Vec<Vec<usize>>>,
    #[serde(default)]
    edges_assignment: Option<Vec<String>>,
    #[serde(default)]
    faces_vertices: Option<Vec<Vec<usize>>>,
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
    #[serde(default)]
    edges_vertices: Option<Vec<Vec<usize>>>,
    #[serde(default)]
    edges_assignment: Option<Vec<String>>,
    #[serde(default)]
    faces_vertices: Option<Vec<Vec<usize>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct Topology {
    edges: Vec<[usize; 2]>,
    assignments: Vec<String>,
    faces: Vec<Vec<usize>>,
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
    let root_topology_raw = (
        raw.edges_vertices.clone(),
        raw.edges_assignment.clone(),
        raw.faces_vertices.clone(),
    );
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
    let root_topology = parse_topology(
        root_topology_raw.0,
        root_topology_raw.1,
        root_topology_raw.2,
        root.as_ref().map_or(0, Vec::len),
        limits,
    )?;
    let mut topologies = vec![None; raw.file_frames.len()];
    let mut topology_visiting = HashSet::new();
    for index in 0..raw.file_frames.len() {
        resolve_topology(
            index,
            &raw.file_frames,
            root_topology.as_ref(),
            &resolved_vertices,
            limits,
            &mut topologies,
            &mut topology_visiting,
        )?;
    }
    let baseline = topologies.iter().flatten().next();
    let topology_sha256 = topologies
        .iter()
        .map(|topology| {
            topology.as_ref().map(|value| {
                Sha256::digest(serde_json::to_vec(value).expect("topology serializes")).into()
            })
        })
        .collect::<Vec<_>>();
    let frames = raw
        .file_frames
        .iter()
        .enumerate()
        .map(|(index, frame)| Fold3dFramePreviewV1 {
            index,
            parent: frame.frame_parent,
            inherits: frame.frame_inherit,
            vertex_count: resolved_vertices[index].len(),
            topology_identity_authenticated: topologies[index]
                .as_ref()
                .zip(baseline)
                .is_some_and(|(current, baseline)| current == baseline),
        })
        .collect();
    Ok(Fold3dFramesPreviewV1 {
        source_sha256: Sha256::digest(bytes).into(),
        frames,
        resolved_vertices,
        topology_sha256,
        topologies,
        rest_vertices: root,
    })
}

fn resolve_topology(
    index: usize,
    frames: &[RawFrame],
    root: Option<&Topology>,
    vertices: &[Vec<[f64; 3]>],
    limits: FoldImportLimits,
    resolved: &mut [Option<Topology>],
    visiting: &mut HashSet<usize>,
) -> Result<(), Fold3dFramesImportErrorV1> {
    if resolved[index].is_some() {
        return Ok(());
    }
    if !visiting.insert(index) {
        return Err(Fold3dFramesImportErrorV1::InvalidFrameParent);
    }
    let frame = &frames[index];
    let own = parse_topology(
        frame.edges_vertices.clone(),
        frame.edges_assignment.clone(),
        frame.faces_vertices.clone(),
        vertices[index].len(),
        limits,
    )?;
    let inherited = if frame.frame_inherit {
        if let Some(parent) = frame.frame_parent {
            resolve_topology(parent, frames, root, vertices, limits, resolved, visiting)?;
            resolved[parent].clone()
        } else {
            root.cloned()
        }
    } else {
        None
    };
    resolved[index] = own.or(inherited);
    visiting.remove(&index);
    Ok(())
}

fn parse_topology(
    edges: Option<Vec<Vec<usize>>>,
    assignments: Option<Vec<String>>,
    faces: Option<Vec<Vec<usize>>>,
    vertex_count: usize,
    limits: FoldImportLimits,
) -> Result<Option<Topology>, Fold3dFramesImportErrorV1> {
    if edges.is_none() && assignments.is_none() && faces.is_none() {
        return Ok(None);
    }
    let (edges, assignments, faces) = (
        edges.ok_or(Fold3dFramesImportErrorV1::Malformed)?,
        assignments.ok_or(Fold3dFramesImportErrorV1::Malformed)?,
        faces.ok_or(Fold3dFramesImportErrorV1::Malformed)?,
    );
    if edges.len() > limits.max_edges
        || edges.len() != assignments.len()
        || faces.len() > limits.max_vertices
    {
        return Err(Fold3dFramesImportErrorV1::TooManyVertices);
    }
    let edges = edges
        .into_iter()
        .map(|edge| {
            let [a, b]: [usize; 2] = edge
                .try_into()
                .map_err(|_| Fold3dFramesImportErrorV1::Malformed)?;
            if a == b || a >= vertex_count || b >= vertex_count {
                return Err(Fold3dFramesImportErrorV1::Malformed);
            }
            Ok([a, b])
        })
        .collect::<Result<Vec<_>, _>>()?;
    if assignments
        .iter()
        .any(|a| !matches!(a.as_str(), "B" | "M" | "V" | "F" | "U" | "C" | "J"))
        || faces
            .iter()
            .any(|face| face.len() < 3 || face.iter().any(|i| *i >= vertex_count))
    {
        return Err(Fold3dFramesImportErrorV1::Malformed);
    }
    Ok(Some(Topology {
        edges,
        assignments,
        faces,
    }))
}

fn prepare_pose(
    rest: &[[f64; 3]],
    target: &[[f64; 3]],
    topology: &Topology,
    source_sha256: [u8; 32],
    frame_index: usize,
) -> Result<Fold3dAppliedPoseProposalV1, Fold3dFramesImportErrorV1> {
    if rest.len() != target.len() || rest.iter().any(|p| p[2].to_bits() != 0.0_f64.to_bits()) {
        return Err(Fold3dFramesImportErrorV1::PoseUnavailable);
    }
    let mut transforms = Vec::with_capacity(topology.faces.len());
    let mut normals = Vec::with_capacity(topology.faces.len());
    for face in &topology.faces {
        let a = face[0];
        let (b, c) = face[1..]
            .iter()
            .copied()
            .zip(face[2..].iter().copied())
            .find(|(b, c)| cross2(rest[a], rest[*b], rest[*c]).abs() > 1e-12)
            .ok_or(Fold3dFramesImportErrorV1::PoseUnavailable)?;
        let ru = normalize3(sub3(rest[b], rest[a]))?;
        let rn = normalize3(cross3(sub3(rest[b], rest[a]), sub3(rest[c], rest[a])))?;
        let rv = cross3(rn, ru);
        let tu = normalize3(sub3(target[b], target[a]))?;
        let tn = normalize3(cross3(
            sub3(target[b], target[a]),
            sub3(target[c], target[a]),
        ))?;
        let tv = cross3(tn, tu);
        let rotation = [
            [
                tu[0] * ru[0] + tv[0] * rv[0] + tn[0] * rn[0],
                tu[0] * ru[1] + tv[0] * rv[1] + tn[0] * rn[1],
                tu[0] * ru[2] + tv[0] * rv[2] + tn[0] * rn[2],
            ],
            [
                tu[1] * ru[0] + tv[1] * rv[0] + tn[1] * rn[0],
                tu[1] * ru[1] + tv[1] * rv[1] + tn[1] * rn[1],
                tu[1] * ru[2] + tv[1] * rv[2] + tn[1] * rn[2],
            ],
            [
                tu[2] * ru[0] + tv[2] * rv[0] + tn[2] * rn[0],
                tu[2] * ru[1] + tv[2] * rv[1] + tn[2] * rn[1],
                tu[2] * ru[2] + tv[2] * rv[2] + tn[2] * rn[2],
            ],
        ];
        let translation = sub3(target[a], mul3(rotation, rest[a]));
        let scale = face.iter().map(|i| norm3(rest[*i])).fold(1.0, f64::max);
        if face.iter().any(|i| {
            norm3(sub3(
                add3(mul3(rotation, rest[*i]), translation),
                target[*i],
            )) > 1e-8 * scale
        }) {
            return Err(Fold3dFramesImportErrorV1::PoseUnavailable);
        }
        transforms.push([
            [
                rotation[0][0],
                rotation[0][1],
                rotation[0][2],
                translation[0],
            ],
            [
                rotation[1][0],
                rotation[1][1],
                rotation[1][2],
                translation[1],
            ],
            [
                rotation[2][0],
                rotation[2][1],
                rotation[2][2],
                translation[2],
            ],
        ]);
        normals.push(tn);
    }
    let mut hinges = Vec::new();
    let mut adjacency = vec![Vec::new(); topology.faces.len()];
    for (edge_index, edge) in topology.edges.iter().enumerate() {
        let incident = topology
            .faces
            .iter()
            .enumerate()
            .filter_map(|(index, face)| {
                face.windows(2)
                    .chain(std::iter::once(&[face[face.len() - 1], face[0]][..]))
                    .any(|pair| {
                        (pair[0] == edge[0] && pair[1] == edge[1])
                            || (pair[0] == edge[1] && pair[1] == edge[0])
                    })
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        match topology.assignments[edge_index].as_str() {
            "B" if incident.len() == 1 => {}
            assignment @ ("M" | "V") if incident.len() == 2 => {
                adjacency[incident[0]].push(incident[1]);
                adjacency[incident[1]].push(incident[0]);
                let axis = normalize3(sub3(target[edge[1]], target[edge[0]]))?;
                let angle = dot3(axis, cross3(normals[incident[0]], normals[incident[1]]))
                    .atan2(dot3(normals[incident[0]], normals[incident[1]]))
                    .to_degrees();
                if (assignment == "M" && angle >= -1e-10) || (assignment == "V" && angle <= 1e-10) {
                    return Err(Fold3dFramesImportErrorV1::PoseUnavailable);
                }
                hinges.push((edge_index, angle));
            }
            _ => return Err(Fold3dFramesImportErrorV1::PoseUnavailable),
        }
    }
    if hinges.len() + 1 != topology.faces.len() || topology.faces.is_empty() {
        return Err(Fold3dFramesImportErrorV1::PoseUnavailable);
    }
    let mut seen = vec![false; topology.faces.len()];
    let mut stack = vec![0];
    while let Some(face) = stack.pop() {
        if seen[face] {
            continue;
        }
        seen[face] = true;
        stack.extend(adjacency[face].iter().copied());
    }
    if seen.iter().any(|seen| !seen) {
        return Err(Fold3dFramesImportErrorV1::PoseUnavailable);
    }
    Ok(Fold3dAppliedPoseProposalV1 {
        source_sha256,
        frame_index,
        face_transforms: transforms,
        hinge_angles_degrees: hinges,
        rest_vertices: rest.to_vec(),
        edges: topology.edges.clone(),
        faces: topology.faces.clone(),
        assignments: topology.assignments.clone(),
    })
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm3(a: [f64; 3]) -> f64 {
    dot3(a, a).sqrt()
}
fn normalize3(a: [f64; 3]) -> Result<[f64; 3], Fold3dFramesImportErrorV1> {
    let n = norm3(a);
    if !n.is_finite() || n <= 1e-12 {
        Err(Fold3dFramesImportErrorV1::PoseUnavailable)
    } else {
        Ok([a[0] / n, a[1] / n, a[2] / n])
    }
}
fn mul3(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [dot3(m[0], v), dot3(m[1], v), dot3(m[2], v)]
}
fn cross2(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
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
        let limits = FoldImportLimits {
            max_vertices: 1,
            ..Default::default()
        };
        let bounded = br#"{"file_frames":[{"vertices_coords":[[0,0,0],[1,0,0]]}]}"#;
        assert_eq!(
            read_fold_3d_frames_preview_v1(bounded, limits),
            Err(Fold3dFramesImportErrorV1::TooManyVertices)
        );
    }

    #[test]
    fn inherited_fold_topology_gets_stable_identity_but_no_pose_authority() {
        let bytes = br#"{
          "vertices_coords":[[0,0,0],[1,0,0],[1,1,0],[0,1,0]],
          "edges_vertices":[[0,1],[1,2],[2,3],[3,0],[0,2]],
          "edges_assignment":["B","B","B","B","M"],
          "faces_vertices":[[0,1,2],[0,2,3]],
          "file_frames":[
            {"frame_inherit":true},
            {"frame_parent":0,"frame_inherit":true,
             "vertices_coords":[[0,0,0],[1,0,0],[1,1,1],[0,1,0]]}
          ]
        }"#;
        let preview = read_fold_3d_frames_preview_v1(bytes, FoldImportLimits::default()).unwrap();
        assert!(
            preview
                .frames()
                .iter()
                .all(Fold3dFramePreviewV1::topology_identity_authenticated)
        );
        let first = preview.select_frame(0).unwrap();
        let second = preview.select_frame(1).unwrap();
        assert_eq!(first.topology_sha256(), second.topology_sha256());
        assert!(first.topology_sha256().is_some());
        assert!(!second.authorizes_applied_pose());
        assert!(!second.authorizes_instruction_timeline());
        assert_eq!(
            preview.prepare_applied_pose_proposal(1),
            Err(Fold3dFramesImportErrorV1::PoseUnavailable)
        );
    }

    #[test]
    fn rigid_two_face_tree_issues_opaque_pose_proposal() {
        let bytes = br#"{
          "vertices_coords":[[0,0,0],[1,0,0],[1,1,0],[0,1,0]],
          "edges_vertices":[[0,1],[1,2],[2,3],[3,0],[0,2]],
          "edges_assignment":["B","B","B","B","V"],
          "faces_vertices":[[0,1,2],[0,2,3]],
          "file_frames":[{"frame_inherit":true,
            "vertices_coords":[[0,0,0],[1,0,0],[1,1,0],[0.5,0.5,0.7071067811865476]]
          }]
        }"#;
        let preview = read_fold_3d_frames_preview_v1(bytes, FoldImportLimits::default()).unwrap();
        let proposal = preview.prepare_applied_pose_proposal(0).unwrap();
        assert_eq!(proposal.face_transforms().len(), 2);
        assert_eq!(proposal.hinge_angles_degrees().len(), 1);
        assert!(proposal.hinge_angles_degrees()[0].1 > 89.999);
        assert!(!proposal.authorizes_project_mutation());
    }
}
