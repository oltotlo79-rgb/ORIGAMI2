use ori_domain::{CreasePattern, EdgeKind, Paper, VertexId};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Domain separator for the first fold-model content fingerprint.
///
/// Changing the encoded fields or their normalization requires a new version
/// and a new separator.
const FOLD_MODEL_FINGERPRINT_V1_DOMAIN: &[u8] = b"ORIGAMI2\0fold-model-fingerprint\0v1\0";
const FINGERPRINT_CHECK_INTERVAL: usize = 64;

/// Stable exact content identity used by proof provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct FoldModelFingerprintV1(pub [u8; 32]);

impl FoldModelFingerprintV1 {
    /// Encodes the digest with the persisted lowercase hexadecimal format.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut encoded = String::with_capacity(64);
        for byte in self.0 {
            use std::fmt::Write as _;
            write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        encoded
    }
}

/// Produces the canonical SHA-256 identity of geometry that determines a fold
/// model.
///
/// Vertex and edge storage order, undirected edge direction, and paper
/// boundary cycle/reversal are normalized. Binary64 coordinates and thickness
/// retain their exact IEEE-754 bits. Appearance and instruction data are
/// deliberately outside this identity.
#[must_use]
pub fn fold_model_fingerprint_v1(pattern: &CreasePattern, paper: &Paper) -> FoldModelFingerprintV1 {
    let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
    match fold_model_fingerprint_v1_with_checkpoint(pattern, paper, &mut checkpoint) {
        Ok(fingerprint) => fingerprint,
        Err(error) => match error {},
    }
}

pub(crate) fn fold_model_fingerprint_v1_with_checkpoint<E, F>(
    pattern: &CreasePattern,
    paper: &Paper,
    checkpoint: &mut F,
) -> Result<FoldModelFingerprintV1, E>
where
    F: FnMut() -> Result<(), E> + ?Sized,
{
    checkpoint()?;
    let mut hasher = Sha256::new();
    hasher.update(FOLD_MODEL_FINGERPRINT_V1_DOMAIN);

    let mut vertices = pattern.vertices.iter().collect::<Vec<_>>();
    vertices.sort_by_key(|vertex| {
        (
            vertex.id.canonical_bytes(),
            vertex.position.x.to_bits(),
            vertex.position.y.to_bits(),
        )
    });
    checkpoint()?;
    hash_len(&mut hasher, vertices.len());
    for (index, vertex) in vertices.into_iter().enumerate() {
        poll_checkpoint(checkpoint, index)?;
        hasher.update(vertex.id.canonical_bytes());
        hasher.update(vertex.position.x.to_bits().to_be_bytes());
        hasher.update(vertex.position.y.to_bits().to_be_bytes());
    }
    checkpoint()?;

    let mut edges = pattern.edges.iter().collect::<Vec<_>>();
    edges.sort_by_key(|edge| {
        let mut endpoints = [edge.start.canonical_bytes(), edge.end.canonical_bytes()];
        endpoints.sort_unstable();
        (
            edge.id.canonical_bytes(),
            endpoints,
            edge_kind_tag(edge.kind),
        )
    });
    checkpoint()?;
    hash_len(&mut hasher, edges.len());
    for (index, edge) in edges.into_iter().enumerate() {
        poll_checkpoint(checkpoint, index)?;
        let mut endpoints = [edge.start.canonical_bytes(), edge.end.canonical_bytes()];
        endpoints.sort_unstable();
        hasher.update(edge.id.canonical_bytes());
        hasher.update(endpoints[0]);
        hasher.update(endpoints[1]);
        hasher.update([edge_kind_tag(edge.kind)]);
    }
    checkpoint()?;

    let boundary = canonical_boundary(&paper.boundary_vertices);
    checkpoint()?;
    hash_len(&mut hasher, boundary.len());
    for (index, vertex) in boundary.into_iter().enumerate() {
        poll_checkpoint(checkpoint, index)?;
        hasher.update(vertex.canonical_bytes());
    }
    hasher.update([u8::from(paper.cutting_allowed)]);
    hasher.update(paper.thickness_mm.to_bits().to_be_bytes());

    checkpoint()?;
    Ok(FoldModelFingerprintV1(hasher.finalize().into()))
}

fn poll_checkpoint<E, F>(checkpoint: &mut F, index: usize) -> Result<(), E>
where
    F: FnMut() -> Result<(), E> + ?Sized,
{
    if index.is_multiple_of(FINGERPRINT_CHECK_INTERVAL) {
        checkpoint()?;
    }
    Ok(())
}

fn hash_len(hasher: &mut Sha256, len: usize) {
    hasher.update(
        u64::try_from(len)
            .expect("a collection length must fit in u64 on supported targets")
            .to_be_bytes(),
    );
}

const fn edge_kind_tag(kind: EdgeKind) -> u8 {
    match kind {
        EdgeKind::Mountain => 0,
        EdgeKind::Valley => 1,
        EdgeKind::Auxiliary => 2,
        EdgeKind::Boundary => 3,
        EdgeKind::Cut => 4,
    }
}

fn canonical_boundary(boundary: &[VertexId]) -> Vec<VertexId> {
    if boundary.len() < 2 {
        return boundary.to_vec();
    }

    let forward_bytes = boundary
        .iter()
        .map(VertexId::canonical_bytes)
        .collect::<Vec<_>>();
    let reverse_bytes = forward_bytes.iter().copied().rev().collect::<Vec<_>>();
    let forward_start = least_rotation_start(&forward_bytes);
    let reverse_start = least_rotation_start(&reverse_bytes);

    let forward_key = rotated(&forward_bytes, forward_start);
    let reverse_key = rotated(&reverse_bytes, reverse_start);
    if forward_key <= reverse_key {
        rotated(boundary, forward_start)
    } else {
        let reversed = boundary.iter().copied().rev().collect::<Vec<_>>();
        rotated(&reversed, reverse_start)
    }
}

/// Booth's algorithm for the lexicographically least cyclic rotation.
fn least_rotation_start<T: Ord>(values: &[T]) -> usize {
    let len = values.len();
    if len < 2 {
        return 0;
    }
    let (mut first, mut second, mut offset) = (0, 1, 0);
    while first < len && second < len && offset < len {
        use std::cmp::Ordering;
        match values[(first + offset) % len].cmp(&values[(second + offset) % len]) {
            Ordering::Equal => offset += 1,
            Ordering::Greater => {
                first += offset + 1;
                if first == second {
                    first += 1;
                }
                offset = 0;
            }
            Ordering::Less => {
                second += offset + 1;
                if first == second {
                    second += 1;
                }
                offset = 0;
            }
        }
    }
    first.min(second) % len
}

fn rotated<T: Copy>(values: &[T], start: usize) -> Vec<T> {
    values[start..]
        .iter()
        .chain(&values[..start])
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, EdgeId, Point2, Vertex};

    use super::*;

    fn fixture() -> (CreasePattern, Paper) {
        let first = VertexId::new();
        let second = VertexId::new();
        let third = VertexId::new();
        (
            CreasePattern {
                vertices: vec![
                    Vertex {
                        id: first,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: second,
                        position: Point2::new(1.0, 0.0),
                    },
                    Vertex {
                        id: third,
                        position: Point2::new(0.0, 1.0),
                    },
                ],
                edges: vec![Edge {
                    id: EdgeId::new(),
                    start: first,
                    end: second,
                    kind: EdgeKind::Mountain,
                }],
            },
            Paper {
                boundary_vertices: vec![first, second, third],
                thickness_mm: 0.1,
                cutting_allowed: false,
                ..Paper::default()
            },
        )
    }

    #[test]
    fn fingerprint_is_deterministic_lowercase_sha256() {
        let (pattern, paper) = fixture();
        let first = fold_model_fingerprint_v1(&pattern, &paper);
        let second = fold_model_fingerprint_v1(&pattern, &paper);
        let encoded = first.to_hex();
        assert_eq!(first, second);
        assert_eq!(encoded.len(), 64);
        assert!(encoded.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(encoded, encoded.to_ascii_lowercase());
    }

    #[test]
    fn storage_order_direction_and_boundary_cycle_are_normalized() {
        let (pattern, paper) = fixture();
        let expected = fold_model_fingerprint_v1(&pattern, &paper);
        let mut reordered_pattern = pattern.clone();
        reordered_pattern.vertices.reverse();
        reordered_pattern.edges.reverse();
        for edge in &mut reordered_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut reordered_paper = paper.clone();
        reordered_paper.boundary_vertices.rotate_left(1);
        reordered_paper.boundary_vertices.reverse();
        assert_eq!(
            fold_model_fingerprint_v1(&reordered_pattern, &reordered_paper),
            expected
        );
    }

    #[test]
    fn least_rotation_matches_exhaustive_reference() {
        for len in 1_u32..=7 {
            for encoded in 0_u32..3_u32.pow(len) {
                let mut remaining = encoded;
                let values = (0..len)
                    .map(|_| {
                        let value = (remaining % 3) as u8;
                        remaining /= 3;
                        value
                    })
                    .collect::<Vec<_>>();
                let expected = (0..values.len())
                    .map(|start| rotated(&values, start))
                    .min()
                    .expect("non-empty sequence has a rotation");
                assert_eq!(rotated(&values, least_rotation_start(&values)), expected);
            }
        }
    }
}
