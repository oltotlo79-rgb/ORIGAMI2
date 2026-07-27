use super::*;

struct DisjointFacesV1 {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointFacesV1 {
    fn prepare(face_count: usize) -> Option<Self> {
        let mut parent = Vec::new();
        parent.try_reserve_exact(face_count).ok()?;
        parent.extend(0..face_count);
        let mut rank = Vec::new();
        rank.try_reserve_exact(face_count).ok()?;
        rank.resize(face_count, 0);
        Some(Self { parent, rank })
    }

    fn find(&mut self, mut face: usize) -> Option<usize> {
        if face >= self.parent.len() {
            return None;
        }
        while *self.parent.get(face)? != face {
            let parent = *self.parent.get(face)?;
            let grandparent = *self.parent.get(parent)?;
            *self.parent.get_mut(face)? = grandparent;
            face = grandparent;
        }
        Some(face)
    }

    fn union(&mut self, first: usize, second: usize) -> Option<()> {
        let mut first = self.find(first)?;
        let mut second = self.find(second)?;
        if first == second {
            return Some(());
        }
        if self.rank[first] < self.rank[second] {
            std::mem::swap(&mut first, &mut second);
        }
        self.parent[second] = first;
        if self.rank[first] == self.rank[second] {
            self.rank[first] = self.rank[first].checked_add(1)?;
        }
        Some(())
    }
}

fn canonical_initial_angle_bits_v1(
    geometry: &MaterialHingeGraphGeometry,
    schedule: &CanonicalCycleScheduleV1,
) -> Option<HashMap<EdgeId, u64>> {
    let initial = schedule.evaluate(0.0)?;
    let mut initial_by_edge = HashMap::new();
    initial_by_edge.try_reserve(geometry.hinges().len()).ok()?;
    for angle in initial.as_slice() {
        let angle_value = angle.angle_degrees();
        if !angle_value.is_finite()
            || !(0.0..=180.0).contains(&angle_value)
            || initial_by_edge
                .insert(angle.edge(), angle_value.to_bits())
                .is_some()
        {
            return None;
        }
    }
    (initial_by_edge.len() == geometry.hinges().len()).then_some(initial_by_edge)
}

pub(super) fn prepare_active_quotient_v1(
    geometry: &MaterialHingeGraphGeometry,
    graph: &AuthenticatedGraphV1,
    schedule: &CanonicalCycleScheduleV1,
) -> Option<(usize, Vec<ActiveQuotientEdgeV1>)> {
    let initial_by_edge = canonical_initial_angle_bits_v1(geometry, schedule)?;
    let mut classes = Vec::new();
    classes.try_reserve_exact(graph.hinges().len()).ok()?;
    let mut disjoint = DisjointFacesV1::prepare(graph.faces().len())?;
    for record in graph.hinges() {
        let hinge = geometry.hinges().get(record.geometry_index())?;
        let edge = hinge.edge();
        schedule.derivative_bound(edge)?;
        let initial_bits = *initial_by_edge.get(&edge)?;
        let initial = f64::from_bits(initial_bits);
        let class = if schedule.is_exact_constant_profile_v1(edge) {
            if initial == 0.0 {
                disjoint.union(record.left(), record.right())?;
                None
            } else {
                Some(ActiveScheduleClassV1::ConstantAngle(initial.to_bits()))
            }
        } else {
            Some(ActiveScheduleClassV1::CollectiveNonconstant)
        };
        classes.push(class);
    }

    // The authenticated face order is canonical. Assigning the first face
    // encountered in each DSU class therefore gives storage-independent
    // quotient vertex indices even though union-by-rank roots are incidental.
    let mut root_to_component = Vec::new();
    root_to_component
        .try_reserve_exact(graph.faces().len())
        .ok()?;
    root_to_component.resize(graph.faces().len(), usize::MAX);
    let mut component_by_face = Vec::new();
    component_by_face
        .try_reserve_exact(graph.faces().len())
        .ok()?;
    component_by_face.resize(graph.faces().len(), usize::MAX);
    let mut component_count = 0usize;
    for (face, component) in component_by_face.iter_mut().enumerate() {
        let root = disjoint.find(face)?;
        if root_to_component[root] == usize::MAX {
            root_to_component[root] = component_count;
            component_count = component_count.checked_add(1)?;
        }
        *component = root_to_component[root];
    }

    let active_count = classes.iter().filter(|class| class.is_some()).count();
    let mut active = Vec::new();
    active.try_reserve_exact(active_count).ok()?;
    for (record, schedule_class) in graph.hinges().iter().zip(classes) {
        let Some(schedule_class) = schedule_class else {
            continue;
        };
        let left = *component_by_face.get(record.left())?;
        let right = *component_by_face.get(record.right())?;
        // A nonidentity transform cannot close a loop whose endpoints were
        // identified by exact-zero constraints.
        if left == right {
            return None;
        }
        let hinge = geometry.hinges().get(record.geometry_index())?;
        let (line, sign) = exact_generator_line_v1(hinge)?;
        if !matches!(sign, -1 | 1) {
            return None;
        }
        active.push(ActiveQuotientEdgeV1 {
            geometry_index: record.geometry_index(),
            edge: hinge.edge(),
            left,
            right,
            schedule_class,
            line,
            sign,
        });
    }
    (!active.is_empty() && component_count >= 2).then_some((component_count, active))
}

pub(super) fn decompose_active_edge_blocks_v1(
    component_count: usize,
    active: &[ActiveQuotientEdgeV1],
) -> Option<Vec<Vec<usize>>> {
    if !(2..=10_001).contains(&component_count)
        || active.is_empty()
        || active.len() > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP
    {
        return None;
    }
    let adjacency_entry_count = active.len().checked_mul(2)?;
    if adjacency_entry_count > ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP * 2 {
        return None;
    }

    let mut degrees = Vec::new();
    degrees.try_reserve_exact(component_count).ok()?;
    degrees.resize(component_count, 0usize);
    for edge in active {
        if edge.left >= component_count || edge.right >= component_count || edge.left == edge.right
        {
            return None;
        }
        degrees[edge.left] = degrees[edge.left].checked_add(1)?;
        degrees[edge.right] = degrees[edge.right].checked_add(1)?;
    }
    if degrees
        .iter()
        .try_fold(0usize, |sum, degree| sum.checked_add(*degree))?
        != adjacency_entry_count
    {
        return None;
    }

    let mut adjacency = Vec::new();
    adjacency.try_reserve_exact(component_count).ok()?;
    for degree in degrees {
        let mut neighbors = Vec::new();
        neighbors.try_reserve_exact(degree).ok()?;
        adjacency.push(neighbors);
    }
    for (edge_index, edge) in active.iter().enumerate() {
        adjacency[edge.left].push((edge.right, edge_index));
        adjacency[edge.right].push((edge.left, edge_index));
    }
    for neighbors in &mut adjacency {
        neighbors
            .sort_unstable_by_key(|(face, edge)| (*face, active[*edge].edge.canonical_bytes()));
    }

    let mut discovery = Vec::new();
    discovery.try_reserve_exact(component_count).ok()?;
    discovery.resize(component_count, 0usize);
    let mut low = Vec::new();
    low.try_reserve_exact(component_count).ok()?;
    low.resize(component_count, 0usize);
    let mut parent_node = Vec::new();
    parent_node.try_reserve_exact(component_count).ok()?;
    parent_node.resize(component_count, None);
    let mut parent_edge = Vec::new();
    parent_edge.try_reserve_exact(component_count).ok()?;
    parent_edge.resize(component_count, None);
    let mut block_by_edge = Vec::new();
    block_by_edge.try_reserve_exact(active.len()).ok()?;
    block_by_edge.resize(active.len(), usize::MAX);
    let mut edge_stack = Vec::new();
    edge_stack.try_reserve_exact(active.len()).ok()?;
    let mut frames = Vec::new();
    frames.try_reserve_exact(component_count).ok()?;

    let mut next_time = 1usize;
    discovery[0] = next_time;
    low[0] = next_time;
    frames.push((0usize, 0usize));
    let mut block_count = 0usize;
    let mut adjacency_work = 0usize;
    while !frames.is_empty() {
        let frame = frames.len().checked_sub(1)?;
        let node = frames[frame].0;
        let neighbor_index = frames[frame].1;
        if neighbor_index < adjacency[node].len() {
            frames[frame].1 = frames[frame].1.checked_add(1)?;
            adjacency_work = adjacency_work.checked_add(1)?;
            if adjacency_work > adjacency_entry_count {
                return None;
            }
            let (next, edge) = adjacency[node][neighbor_index];
            // Parent edge identity, rather than parent vertex identity, is
            // essential: a second parallel edge must become a back edge.
            if parent_edge[node] == Some(edge) {
                continue;
            }
            if discovery[next] == 0 {
                edge_stack.push(edge);
                next_time = next_time.checked_add(1)?;
                discovery[next] = next_time;
                low[next] = next_time;
                parent_node[next] = Some(node);
                parent_edge[next] = Some(edge);
                frames.push((next, 0));
            } else if discovery[next] < discovery[node] {
                edge_stack.push(edge);
                low[node] = low[node].min(discovery[next]);
            }
        } else {
            frames.pop();
            if let (Some(parent), Some(edge)) = (parent_node[node], parent_edge[node]) {
                if low[node] >= discovery[parent] {
                    if block_count >= active.len() {
                        return None;
                    }
                    loop {
                        let popped = edge_stack.pop()?;
                        if block_by_edge[popped] != usize::MAX {
                            return None;
                        }
                        block_by_edge[popped] = block_count;
                        if popped == edge {
                            break;
                        }
                    }
                    block_count = block_count.checked_add(1)?;
                }
                low[parent] = low[parent].min(low[node]);
            }
        }
    }
    if adjacency_work != adjacency_entry_count
        || discovery.contains(&0)
        || !edge_stack.is_empty()
        || block_count == 0
        || block_by_edge.contains(&usize::MAX)
    {
        return None;
    }

    let mut block_sizes = Vec::new();
    block_sizes.try_reserve_exact(block_count).ok()?;
    block_sizes.resize(block_count, 0usize);
    for block in &block_by_edge {
        block_sizes[*block] = block_sizes[*block].checked_add(1)?;
    }
    if block_sizes
        .iter()
        .try_fold(0usize, |sum, size| sum.checked_add(*size))?
        != active.len()
    {
        return None;
    }
    let mut blocks = Vec::new();
    blocks.try_reserve_exact(block_count).ok()?;
    for size in block_sizes {
        let mut block = Vec::new();
        block.try_reserve_exact(size).ok()?;
        blocks.push(block);
    }
    for (edge, block) in block_by_edge.into_iter().enumerate() {
        blocks[block].push(edge);
    }
    for block in &mut blocks {
        block.sort_unstable_by_key(|edge| active[*edge].edge.canonical_bytes());
    }
    blocks.sort_unstable_by_key(|block| active[block[0]].edge.canonical_bytes());
    Some(blocks)
}

pub(super) fn block_vertices_v1(
    active: &[ActiveQuotientEdgeV1],
    block: &[usize],
) -> Option<Vec<usize>> {
    let capacity = block.len().checked_mul(2)?;
    let mut vertices = Vec::new();
    vertices.try_reserve_exact(capacity).ok()?;
    for edge in block {
        let edge = active.get(*edge)?;
        vertices.push(edge.left);
        vertices.push(edge.right);
    }
    vertices.sort_unstable();
    vertices.dedup();
    (!vertices.is_empty()).then_some(vertices)
}
