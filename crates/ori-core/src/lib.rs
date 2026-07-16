use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};

#[must_use]
pub fn benchmark_pattern(edge_count: usize) -> CreasePattern {
    if edge_count == 0 {
        return CreasePattern::empty();
    }
    let mut side = ((edge_count as f64 / 2.0).sqrt().ceil() as usize).max(2);
    while 2 * side * (side - 1) < edge_count {
        side += 1;
    }
    let mut vertices = Vec::with_capacity(side * side);
    for y in 0..side {
        for x in 0..side {
            vertices.push(Vertex {
                id: VertexId::new(),
                position: Point2::new(x as f64, y as f64),
            });
        }
    }
    let mut edges = Vec::with_capacity(edge_count);
    'grid: for y in 0..side {
        for x in 0..side {
            let index = y * side + x;
            if x + 1 < side {
                edges.push(Edge {
                    id: EdgeId::new(),
                    start: vertices[index].id,
                    end: vertices[index + 1].id,
                    kind: if y % 2 == 0 {
                        EdgeKind::Mountain
                    } else {
                        EdgeKind::Valley
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
            if y + 1 < side {
                edges.push(Edge {
                    id: EdgeId::new(),
                    start: vertices[index].id,
                    end: vertices[index + side].id,
                    kind: if x % 2 == 0 {
                        EdgeKind::Valley
                    } else {
                        EdgeKind::Mountain
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
        }
    }
    CreasePattern { vertices, edges }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_ten_thousand_edge_fixture() {
        let pattern = benchmark_pattern(10_000);
        assert_eq!(pattern.edges.len(), 10_000);
    }
}
