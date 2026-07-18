use ori_domain::{CreasePattern, EdgeKind, Paper, VertexId};
use sha2::{Digest, Sha256};

/// Domain separator for the first persisted fold-model fingerprint format.
///
/// Changing the encoded fields or their normalization requires a new version
/// and a new separator. This prevents the same byte stream from being confused
/// with a hash used for another purpose.
const FOLD_MODEL_FINGERPRINT_V1_DOMAIN: &[u8] = b"ORIGAMI2\0fold-model-fingerprint\0v1\0";

/// Produces the stable SHA-256 identity of the geometry that determines a fold
/// model.
///
/// The input is normalized as follows:
///
/// - vertex and edge records are ordered by canonical UUID bytes;
/// - edge endpoints are ordered because crease edges are undirected;
/// - the paper boundary is normalized across cyclic rotation and reversal;
/// - `f64` values use their exact IEEE-754 bits (including signed zero).
///
/// Project identity, revision, instruction data, names, colours, and texture
/// assets are intentionally outside this fingerprint's domain.
#[must_use]
pub fn fold_model_fingerprint_v1(pattern: &CreasePattern, paper: &Paper) -> String {
    let mut hasher = Sha256::new();
    hasher.update(FOLD_MODEL_FINGERPRINT_V1_DOMAIN);

    let mut vertices: Vec<_> = pattern.vertices.iter().collect();
    vertices.sort_by_key(|vertex| {
        (
            vertex.id.canonical_bytes(),
            vertex.position.x.to_bits(),
            vertex.position.y.to_bits(),
        )
    });
    hash_len(&mut hasher, vertices.len());
    for vertex in vertices {
        hasher.update(vertex.id.canonical_bytes());
        hasher.update(vertex.position.x.to_bits().to_be_bytes());
        hasher.update(vertex.position.y.to_bits().to_be_bytes());
    }

    let mut edges: Vec<_> = pattern.edges.iter().collect();
    edges.sort_by_key(|edge| {
        let mut endpoints = [edge.start.canonical_bytes(), edge.end.canonical_bytes()];
        endpoints.sort_unstable();
        (
            edge.id.canonical_bytes(),
            endpoints,
            edge_kind_tag(edge.kind),
        )
    });
    hash_len(&mut hasher, edges.len());
    for edge in edges {
        let mut endpoints = [edge.start.canonical_bytes(), edge.end.canonical_bytes()];
        endpoints.sort_unstable();
        hasher.update(edge.id.canonical_bytes());
        hasher.update(endpoints[0]);
        hasher.update(endpoints[1]);
        hasher.update([edge_kind_tag(edge.kind)]);
    }

    let boundary = canonical_boundary(&paper.boundary_vertices);
    hash_len(&mut hasher, boundary.len());
    for vertex in boundary {
        hasher.update(vertex.canonical_bytes());
    }
    hasher.update([u8::from(paper.cutting_allowed)]);
    hasher.update(paper.thickness_mm.to_bits().to_be_bytes());

    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
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

    let forward_bytes: Vec<_> = boundary.iter().map(VertexId::canonical_bytes).collect();
    let reverse_bytes: Vec<_> = forward_bytes.iter().copied().rev().collect();
    let forward_start = least_rotation_start(&forward_bytes);
    let reverse_start = least_rotation_start(&reverse_bytes);

    let forward_key = rotated(&forward_bytes, forward_start);
    let reverse_key = rotated(&reverse_bytes, reverse_start);
    if forward_key <= reverse_key {
        rotated(boundary, forward_start)
    } else {
        let reversed: Vec<_> = boundary.iter().copied().rev().collect();
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
    use super::*;
    use ori_domain::{Edge, EdgeId, Point2, Vertex};

    fn fixture() -> (CreasePattern, Paper) {
        let first = VertexId::new();
        let second = VertexId::new();
        let third = VertexId::new();
        let edge = EdgeId::new();
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
                    id: edge,
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
    fn fingerprint_is_lowercase_sha256_and_deterministic() {
        let (pattern, paper) = fixture();
        let first = fold_model_fingerprint_v1(&pattern, &paper);
        let second = fold_model_fingerprint_v1(&pattern, &paper);

        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(first, first.to_ascii_lowercase());
    }

    #[test]
    fn storage_order_edge_direction_and_boundary_cycle_are_normalized() {
        let (pattern, paper) = fixture();
        let expected = fold_model_fingerprint_v1(&pattern, &paper);

        let mut reordered_pattern = pattern.clone();
        reordered_pattern.vertices.reverse();
        reordered_pattern.edges.reverse();
        for edge in &mut reordered_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }
        let mut rotated_paper = paper.clone();
        rotated_paper.boundary_vertices.rotate_left(1);
        rotated_paper.boundary_vertices.reverse();

        assert_eq!(
            fold_model_fingerprint_v1(&reordered_pattern, &rotated_paper),
            expected
        );
    }

    #[test]
    fn least_rotation_matches_exhaustive_reference_with_duplicate_values() {
        for len in 1_u32..=7 {
            for encoded in 0_u32..3_u32.pow(len) {
                let mut remaining = encoded;
                let values: Vec<_> = (0..len)
                    .map(|_| {
                        let value = (remaining % 3) as u8;
                        remaining /= 3;
                        value
                    })
                    .collect();
                let expected = (0..values.len())
                    .map(|start| rotated(&values, start))
                    .min()
                    .expect("non-empty sequence has a rotation");
                let actual = rotated(&values, least_rotation_start(&values));
                assert_eq!(actual, expected, "values: {values:?}");
            }
        }
    }

    #[test]
    fn duplicate_invalid_records_do_not_reintroduce_storage_order_dependence() {
        let (mut pattern, paper) = fixture();
        pattern.vertices.push(Vertex {
            id: pattern.vertices[0].id,
            position: Point2::new(9.0, 8.0),
        });
        pattern.edges.push(Edge {
            id: pattern.edges[0].id,
            start: pattern.vertices[2].id,
            end: pattern.vertices[1].id,
            kind: EdgeKind::Auxiliary,
        });
        let expected = fold_model_fingerprint_v1(&pattern, &paper);
        pattern.vertices.reverse();
        pattern.edges.reverse();

        assert_eq!(fold_model_fingerprint_v1(&pattern, &paper), expected);
    }

    #[test]
    fn every_fold_model_field_changes_the_fingerprint() {
        let (pattern, paper) = fixture();
        let expected = fold_model_fingerprint_v1(&pattern, &paper);

        let mut changed = pattern.clone();
        changed.vertices[0].position.x = -0.0;
        assert_ne!(fold_model_fingerprint_v1(&changed, &paper), expected);

        let mut changed = pattern.clone();
        changed.edges[0].kind = EdgeKind::Valley;
        assert_ne!(fold_model_fingerprint_v1(&changed, &paper), expected);

        let mut changed = paper.clone();
        changed.cutting_allowed = true;
        assert_ne!(fold_model_fingerprint_v1(&pattern, &changed), expected);

        let mut changed = paper.clone();
        changed.thickness_mm = 0.2;
        assert_ne!(fold_model_fingerprint_v1(&pattern, &changed), expected);

        let mut changed = paper.clone();
        changed.boundary_vertices[0] = VertexId::new();
        assert_ne!(fold_model_fingerprint_v1(&pattern, &changed), expected);
    }

    #[test]
    fn appearance_is_outside_the_fingerprint_domain() {
        let (pattern, paper) = fixture();
        let expected = fold_model_fingerprint_v1(&pattern, &paper);
        let mut changed = paper.clone();
        changed.front.color.red = 1;
        changed.back.color.alpha = 2;

        assert_eq!(fold_model_fingerprint_v1(&pattern, &changed), expected);
    }
}
