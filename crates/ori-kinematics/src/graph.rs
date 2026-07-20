use std::collections::{HashMap, HashSet};

use ori_domain::{EdgeId, FaceId};
use ori_topology::TopologySnapshot;

use crate::{KinematicsError, TreeKinematicsLimits};

/// A bounded, geometry-free audit of a connected material hinge graph.
///
/// This is deliberately not a pose authority.  In particular, closure hinges
/// identify constraints that a future general graph solver must prove; they
/// must never be silently discarded and solved as an unconstrained tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialHingeGraphAudit {
    faces: Vec<FaceId>,
    spanning_hinges: Vec<EdgeId>,
    closure_hinges: Vec<EdgeId>,
}

impl MaterialHingeGraphAudit {
    /// Validates connectivity and deterministically partitions hinges into a
    /// canonical spanning tree and the remaining loop-closure constraints.
    pub fn prepare(
        topology: &TopologySnapshot,
        limits: TreeKinematicsLimits,
    ) -> Result<Self, KinematicsError> {
        if topology.faces.is_empty()
            || topology.faces.len() > limits.max_faces
            || topology.hinge_adjacency.len() > limits.max_hinges
            || topology
                .hinge_adjacency
                .len()
                .checked_mul(2)
                .is_none_or(|entries| entries > limits.max_adjacency_entries)
        {
            return Err(
                if topology.faces.len() > limits.max_faces
                    || topology.hinge_adjacency.len() > limits.max_hinges
                {
                    KinematicsError::ResourceLimitExceeded
                } else {
                    KinematicsError::UnsupportedTopology
                },
            );
        }

        let mut faces = topology
            .faces
            .iter()
            .map(|face| face.id)
            .collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        if faces.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(KinematicsError::UnsupportedTopology);
        }

        let indices = faces
            .iter()
            .copied()
            .enumerate()
            .map(|(index, face)| (face, index))
            .collect::<HashMap<_, _>>();
        let mut hinges = topology.hinge_adjacency.iter().collect::<Vec<_>>();
        hinges.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
        let mut edge_ids = HashSet::with_capacity(hinges.len());
        let mut parent = (0..faces.len()).collect::<Vec<_>>();
        let mut rank = vec![0_u8; faces.len()];
        let mut spanning_hinges = Vec::with_capacity(faces.len().saturating_sub(1));
        let mut closure_hinges = Vec::new();

        for hinge in hinges {
            if hinge.first == hinge.second || !edge_ids.insert(hinge.edge) {
                return Err(KinematicsError::UnsupportedTopology);
            }
            let first = *indices
                .get(&hinge.first)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            let second = *indices
                .get(&hinge.second)
                .ok_or(KinematicsError::UnsupportedTopology)?;
            if union(&mut parent, &mut rank, first, second) {
                spanning_hinges.push(hinge.edge);
            } else {
                closure_hinges.push(hinge.edge);
            }
        }
        if spanning_hinges.len() != faces.len().saturating_sub(1) {
            return Err(KinematicsError::UnsupportedTopology);
        }
        Ok(Self {
            faces,
            spanning_hinges,
            closure_hinges,
        })
    }

    #[must_use]
    pub fn faces(&self) -> &[FaceId] {
        &self.faces
    }

    #[must_use]
    pub fn spanning_hinges(&self) -> &[EdgeId] {
        &self.spanning_hinges
    }

    #[must_use]
    pub fn closure_hinges(&self) -> &[EdgeId] {
        &self.closure_hinges
    }

    #[must_use]
    pub const fn is_tree(&self) -> bool {
        self.closure_hinges.is_empty()
    }
}

fn find(parent: &mut [usize], mut node: usize) -> usize {
    while parent[node] != node {
        parent[node] = parent[parent[node]];
        node = parent[node];
    }
    node
}

fn union(parent: &mut [usize], rank: &mut [u8], first: usize, second: usize) -> bool {
    let mut first = find(parent, first);
    let mut second = find(parent, second);
    if first == second {
        return false;
    }
    if rank[first] < rank[second] {
        std::mem::swap(&mut first, &mut second);
    }
    parent[second] = first;
    if rank[first] == rank[second] {
        rank[first] = rank[first].saturating_add(1);
    }
    true
}

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeId, FaceId, ProjectId};
    use ori_topology::{
        BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot,
    };

    use super::*;

    fn face(id: FaceId) -> Face {
        Face {
            id,
            key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
            outer: BoundaryWalk {
                half_edges: Vec::new(),
                signed_double_area: 1.0,
            },
            holes: Vec::new(),
            seams: Vec::new(),
            area: 0.5,
        }
    }

    fn topology(faces: &[FaceId], hinges: &[(EdgeId, FaceId, FaceId)]) -> TopologySnapshot {
        TopologySnapshot {
            source_revision: 1,
            faces: faces.iter().copied().map(face).collect(),
            edge_incidence: Vec::new(),
            hinge_adjacency: hinges
                .iter()
                .map(|(edge, first, second)| FaceAdjacency {
                    edge: *edge,
                    first: *first,
                    second: *second,
                    assignment: FoldAssignment::Mountain,
                })
                .collect(),
            material_components: Vec::new(),
        }
    }

    #[test]
    fn canonical_partition_exposes_cycle_constraints_without_pose_authority() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"a");
        let b = FaceId::derive_v5(namespace, b"b");
        let c = FaceId::derive_v5(namespace, b"c");
        let ab = EdgeId::derive_v5(namespace, b"ab");
        let bc = EdgeId::derive_v5(namespace, b"bc");
        let ca = EdgeId::derive_v5(namespace, b"ca");
        let baseline = topology(&[a, b, c], &[(ab, a, b), (bc, b, c), (ca, c, a)]);
        let mut reordered = topology(&[c, a, b], &[(ca, c, a), (ab, a, b), (bc, b, c)]);

        let expected =
            MaterialHingeGraphAudit::prepare(&baseline, TreeKinematicsLimits::default()).unwrap();
        let actual =
            MaterialHingeGraphAudit::prepare(&reordered, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(actual, expected);
        assert!(!actual.is_tree());
        assert_eq!(actual.spanning_hinges().len(), 2);
        assert_eq!(actual.closure_hinges().len(), 1);

        reordered.hinge_adjacency.pop();
        let tree =
            MaterialHingeGraphAudit::prepare(&reordered, TreeKinematicsLimits::default()).unwrap();
        assert!(tree.is_tree());
    }

    #[test]
    fn disconnected_duplicate_and_bounded_inputs_fail_closed() {
        let namespace = ProjectId::new();
        let a = FaceId::derive_v5(namespace, b"a");
        let b = FaceId::derive_v5(namespace, b"b");
        let c = FaceId::derive_v5(namespace, b"c");
        let ab = EdgeId::derive_v5(namespace, b"ab");
        let disconnected = topology(&[a, b, c], &[(ab, a, b)]);
        assert_eq!(
            MaterialHingeGraphAudit::prepare(&disconnected, TreeKinematicsLimits::default()),
            Err(KinematicsError::UnsupportedTopology)
        );

        let duplicate = topology(&[a, b], &[(ab, a, b), (ab, a, b)]);
        assert_eq!(
            MaterialHingeGraphAudit::prepare(&duplicate, TreeKinematicsLimits::default()),
            Err(KinematicsError::UnsupportedTopology)
        );

        let mut limits = TreeKinematicsLimits::default();
        limits.max_faces = 1;
        assert_eq!(
            MaterialHingeGraphAudit::prepare(&topology(&[a, b], &[(ab, a, b)]), limits),
            Err(KinematicsError::ResourceLimitExceeded)
        );
    }
}
