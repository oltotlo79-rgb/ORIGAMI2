//! Private measurement of the exact difference between the native binary64
//! material pose and the rational Cayley tree pose.
//!
//! This module deliberately issues no admitted proof, safety decision, public
//! model identifier, renderer payload, or collision-classifier input.  The
//! measured radii remain an internal observation until the numeric admission
//! policy and its hard ceiling are approved and versioned.

use std::collections::{HashMap, VecDeque};

use ori_kinematics::{BoundMaterialTreePose, Point3, RigidTransform};

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeasuredEnvelopeLimits {
    max_faces: usize,
    max_hinges: usize,
    max_adjacency_entries: usize,
    max_depth: usize,
    max_total_depth: usize,
    max_transform_scalars: usize,
    max_boundary_occurrences: usize,
    max_point_components: usize,
    max_unique_vertices: usize,
    max_shared_occurrence_checks: usize,
    max_hinge_feature_points: usize,
    max_exact_hinge_path_checks: usize,
    max_binary64_hinge_path_checks: usize,
    max_hinge_component_checks: usize,
    max_hinge_transform_checks: usize,
    max_certificate_reads: usize,
    max_exact_point_transforms: usize,
    max_input_scalars: usize,
    max_input_bits: usize,
    max_total_input_bits: usize,
    max_output_bits: usize,
    max_total_output_bits: usize,
    exact: CayleyLimits,
}

impl Default for MeasuredEnvelopeLimits {
    fn default() -> Self {
        let mut exact = ExactTreePoseLimits::default().cayley;
        exact.max_interval_operations = 64_000_000;
        Self {
            max_faces: 10_001,
            max_hinges: 10_000,
            max_adjacency_entries: 10_000,
            max_depth: 10_000,
            max_total_depth: 50_005_000,
            max_transform_scalars: 120_012,
            max_boundary_occurrences: 1_000_000,
            max_point_components: 3_000_000,
            max_unique_vertices: 1_000_000,
            max_shared_occurrence_checks: 1_000_000,
            max_hinge_feature_points: 30_000,
            max_exact_hinge_path_checks: 90_000,
            max_binary64_hinge_path_checks: 90_000,
            max_hinge_component_checks: 270_000,
            max_hinge_transform_checks: 10_000,
            max_certificate_reads: 10_000,
            max_exact_point_transforms: 2_150_000,
            max_input_scalars: 3_130_012,
            max_input_bits: 16_384,
            max_total_input_bits: 256_000_000,
            max_output_bits: 16_384,
            max_total_output_bits: 512_000_000,
            exact,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MeasuredEnvelopeWork {
    faces: usize,
    hinges: usize,
    adjacency_entries: usize,
    max_depth: usize,
    total_depth: usize,
    transform_scalars: usize,
    boundary_occurrences: usize,
    point_components: usize,
    unique_vertices: usize,
    shared_occurrence_checks: usize,
    hinge_feature_points: usize,
    exact_hinge_path_checks: usize,
    binary64_hinge_path_checks: usize,
    hinge_component_checks: usize,
    hinge_transform_checks: usize,
    certificate_reads: usize,
    exact_point_transforms: usize,
    input_scalars: usize,
    max_input_bits: usize,
    total_input_bits: usize,
    max_output_bits: usize,
    total_output_bits: usize,
    exact: CayleyWork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MeasuredFaceEnvelope {
    face: FaceId,
    depth: usize,
    radius: [BigRational; 3],
}

#[derive(Debug)]
struct MeasuredBinary64AffineEnvelope<'a> {
    bound: BoundMaterialTreePose<'a>,
    faces: Vec<MeasuredFaceEnvelope>,
    work: MeasuredEnvelopeWork,
}

impl MeasuredBinary64AffineEnvelope<'_> {
    fn is_for(&self, bound: BoundMaterialTreePose<'_>) -> bool {
        self.bound.model() == bound.model() && self.bound.pose().same_instance(bound.pose())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MeasuredEnvelopeError {
    AuthorityMismatch,
    InconsistentPose,
    ResourceLimitExceeded { resource: &'static str },
}

#[derive(Debug, Clone)]
struct VertexWitness {
    rest: ExactPoint3,
    first_face_index: usize,
    first_boundary_index: usize,
}

struct EnvelopeMeter<'a> {
    limits: &'a MeasuredEnvelopeLimits,
    work: MeasuredEnvelopeWork,
    exact: WorkMeter<'a>,
}

impl<'a> EnvelopeMeter<'a> {
    fn new(limits: &'a MeasuredEnvelopeLimits) -> Self {
        Self {
            limits,
            work: MeasuredEnvelopeWork::default(),
            exact: WorkMeter::new(&limits.exact),
        }
    }

    fn input_scalar(&mut self, value: f64) -> Result<BigRational, MeasuredEnvelopeError> {
        // Reserve before conversion so a one-short limit never performs
        // uncharged exact allocation or arithmetic.
        increment(
            &mut self.work.input_scalars,
            self.limits.max_input_scalars,
            "input_scalars",
        )?;
        let value = map_exact(exact_f64(value, &mut self.exact, CayleyStage::Containment))?;
        let bits = rational_bits(&value);
        self.work.max_input_bits = self.work.max_input_bits.max(bits);
        if bits > self.limits.max_input_bits {
            return Err(resource("input_bits"));
        }
        let storage_bits = rational_storage_bits(&value, "total_input_bits")?;
        add_bounded(
            &mut self.work.total_input_bits,
            storage_bits,
            self.limits.max_total_input_bits,
            "total_input_bits",
        )?;
        Ok(value)
    }

    fn source_point(&mut self, point: Point3) -> Result<ExactPoint3, MeasuredEnvelopeError> {
        Ok(ExactPoint3 {
            coordinates: [
                self.input_scalar(point.x())?,
                self.input_scalar(point.y())?,
                self.input_scalar(point.z())?,
            ],
        })
    }

    fn binary64_transform(
        &mut self,
        transform: RigidTransform,
    ) -> Result<ExactRigidTransform, MeasuredEnvelopeError> {
        let rows = transform.rotation_rows();
        let translation = transform.translation();
        let rotation = [
            [
                self.transform_scalar(rows[0][0])?,
                self.transform_scalar(rows[0][1])?,
                self.transform_scalar(rows[0][2])?,
            ],
            [
                self.transform_scalar(rows[1][0])?,
                self.transform_scalar(rows[1][1])?,
                self.transform_scalar(rows[1][2])?,
            ],
            [
                self.transform_scalar(rows[2][0])?,
                self.transform_scalar(rows[2][1])?,
                self.transform_scalar(rows[2][2])?,
            ],
        ];
        let translation = ExactVector3 {
            coordinates: [
                self.transform_scalar(translation.x())?,
                self.transform_scalar(translation.y())?,
                self.transform_scalar(translation.z())?,
            ],
        };
        Ok(ExactRigidTransform {
            rotation,
            translation,
        })
    }

    fn transform_scalar(&mut self, value: f64) -> Result<BigRational, MeasuredEnvelopeError> {
        increment(
            &mut self.work.transform_scalars,
            self.limits.max_transform_scalars,
            "transform_scalars",
        )?;
        self.input_scalar(value)
    }

    fn transform_point(
        &mut self,
        transform: &ExactRigidTransform,
        point: &ExactPoint3,
    ) -> Result<ExactPoint3, MeasuredEnvelopeError> {
        increment(
            &mut self.work.exact_point_transforms,
            self.limits.max_exact_point_transforms,
            "exact_point_transforms",
        )?;
        map_exact(apply_point_at_stage(
            &transform.rotation,
            &transform.translation,
            point,
            &mut self.exact,
            CayleyStage::Containment,
        ))
    }

    fn difference(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, MeasuredEnvelopeError> {
        map_exact(
            self.exact
                .subtract_rational(left, right, CayleyStage::Containment),
        )
        .map(|value| value.abs())
    }

    fn midpoint(
        &mut self,
        first: &ExactPoint3,
        second: &ExactPoint3,
    ) -> Result<ExactPoint3, MeasuredEnvelopeError> {
        let two = BigRational::from_integer(BigInt::from(2_u8));
        let mut coordinates = Vec::new();
        coordinates
            .try_reserve_exact(3)
            .map_err(|_| resource("exact_point_transforms"))?;
        for index in 0..3 {
            let sum = map_exact(self.exact.add_rational(
                &first.coordinates[index],
                &second.coordinates[index],
                CayleyStage::Containment,
            ))?;
            coordinates.push(map_exact(self.exact.divide_rational(
                &sum,
                &two,
                CayleyStage::Containment,
            ))?);
        }
        let coordinates: [BigRational; 3] = coordinates
            .try_into()
            .map_err(|_| MeasuredEnvelopeError::InconsistentPose)?;
        Ok(ExactPoint3 { coordinates })
    }

    fn charge_output(&mut self, value: &BigRational) -> Result<(), MeasuredEnvelopeError> {
        let bits = rational_bits(value);
        self.work.max_output_bits = self.work.max_output_bits.max(bits);
        if bits > self.limits.max_output_bits {
            return Err(resource("output_bits"));
        }
        let storage_bits = rational_storage_bits(value, "total_output_bits")?;
        add_bounded(
            &mut self.work.total_output_bits,
            storage_bits,
            self.limits.max_total_output_bits,
            "total_output_bits",
        )
    }
}

fn measure_binary64_affine_envelope<'a>(
    exact: &RationalCayleyTreePose<'_>,
    observed: BoundMaterialTreePose<'a>,
    limits: MeasuredEnvelopeLimits,
) -> Result<MeasuredBinary64AffineEnvelope<'a>, MeasuredEnvelopeError> {
    if !exact.is_for(observed) {
        return Err(MeasuredEnvelopeError::AuthorityMismatch);
    }
    if exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || exact.fixed_face != observed.pose().fixed_face()
    {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }
    let model = observed.model();
    let pose = observed.pose();
    let face_ids = model.face_ids();
    let hinges = model.hinges();
    let angles = pose.hinge_angles();
    let face_count = face_ids.len();
    let hinge_count = hinges.len();
    check_count(face_count, limits.max_faces, "faces")?;
    check_count(hinge_count, limits.max_hinges, "hinges")?;
    if face_count == 0
        || exact.faces.len() != face_count
        || exact.hinges.len() != hinge_count
        || angles.len() != hinge_count
        || exact.work.faces != face_count
        || exact.work.hinges != hinge_count
        || !strictly_canonical_faces(face_ids)
        || !strictly_canonical_hinges(hinges)
    {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }
    let transform_scalars = checked_product(face_count, 12, "transform_scalars")?;
    check_count(
        transform_scalars,
        limits.max_transform_scalars,
        "transform_scalars",
    )?;
    let adjacency_entries = hinge_count;
    check_count(
        adjacency_entries,
        limits.max_adjacency_entries,
        "adjacency_entries",
    )?;
    let hinge_feature_points = checked_product(hinge_count, 3, "hinge_feature_points")?;
    check_count(
        hinge_feature_points,
        limits.max_hinge_feature_points,
        "hinge_feature_points",
    )?;
    let exact_hinge_path_checks =
        checked_product(hinge_feature_points, 3, "exact_hinge_path_checks")?;
    check_count(
        exact_hinge_path_checks,
        limits.max_exact_hinge_path_checks,
        "exact_hinge_path_checks",
    )?;
    let binary64_hinge_path_checks =
        checked_product(hinge_feature_points, 3, "binary64_hinge_path_checks")?;
    check_count(
        binary64_hinge_path_checks,
        limits.max_binary64_hinge_path_checks,
        "binary64_hinge_path_checks",
    )?;
    let hinge_component_checks =
        checked_product(binary64_hinge_path_checks, 3, "hinge_component_checks")?;
    check_count(
        hinge_component_checks,
        limits.max_hinge_component_checks,
        "hinge_component_checks",
    )?;
    check_count(
        hinge_count,
        limits.max_hinge_transform_checks,
        "hinge_transform_checks",
    )?;
    check_count(
        hinge_count,
        limits.max_certificate_reads,
        "certificate_reads",
    )?;

    let mut boundary_occurrences = 0_usize;
    for (index, face) in exact.faces.iter().enumerate() {
        if face.face != face_ids[index] {
            return Err(MeasuredEnvelopeError::InconsistentPose);
        }
        let boundary = observed
            .face_boundary(face.face)
            .filter(|boundary| model.owns_face_boundary(*boundary))
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        if boundary.face() != face.face
            || boundary.vertices().len() != boundary.edges().len()
            || boundary.vertices().len() < 3
            || face.boundary.len() != boundary.vertices().len()
            || !face
                .boundary
                .iter()
                .zip(boundary.vertices())
                .all(|((vertex, _), expected)| vertex == expected)
        {
            return Err(MeasuredEnvelopeError::InconsistentPose);
        }
        boundary_occurrences = boundary_occurrences
            .checked_add(boundary.vertices().len())
            .ok_or_else(|| resource("boundary_occurrences"))?;
        check_count(
            boundary_occurrences,
            limits.max_boundary_occurrences,
            "boundary_occurrences",
        )?;
    }
    if boundary_occurrences != exact.work.boundary_occurrences {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }
    if exact.work.unique_vertices > boundary_occurrences {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }
    let point_components = checked_product(boundary_occurrences, 3, "point_components")?;
    check_count(
        point_components,
        limits.max_point_components,
        "point_components",
    )?;
    let boundary_point_transforms =
        checked_product(boundary_occurrences, 2, "exact_point_transforms")?;
    let hinge_point_transforms =
        checked_product(hinge_feature_points, 5, "exact_point_transforms")?;
    let expected_point_transforms = boundary_point_transforms
        .checked_add(hinge_point_transforms)
        .ok_or_else(|| resource("exact_point_transforms"))?;
    check_count(
        expected_point_transforms,
        limits.max_exact_point_transforms,
        "exact_point_transforms",
    )?;
    let source_scalars = checked_product(exact.work.unique_vertices, 3, "input_scalars")?;
    let expected_input_scalars = transform_scalars
        .checked_add(source_scalars)
        .and_then(|count| count.checked_add(hinge_count))
        .ok_or_else(|| resource("input_scalars"))?;
    check_count(
        expected_input_scalars,
        limits.max_input_scalars,
        "input_scalars",
    )?;

    let mut meter = EnvelopeMeter::new(&limits);
    meter.work.faces = face_count;
    meter.work.hinges = hinge_count;
    meter.work.adjacency_entries = adjacency_entries;

    let depths = authenticate_tree_and_depths(exact, observed, &mut meter)?;

    let mut binary64_transforms = Vec::new();
    binary64_transforms
        .try_reserve_exact(face_count)
        .map_err(|_| resource("faces"))?;
    // Each retained transform/source rational is moved exactly once into this
    // cache or the vertex registry below. The certificate angle is temporary
    // but conservatively charged too, so `total_input_bits` bounds retained
    // input storage and its conversion scratch without an unmetered duplicate.
    for face in &exact.faces {
        let transform = pose
            .face_transform(face.face)
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        binary64_transforms.push(meter.binary64_transform(transform)?);
    }
    let mut vertex_registry = HashMap::<VertexId, VertexWitness>::new();
    vertex_registry
        .try_reserve(boundary_occurrences.min(limits.max_unique_vertices))
        .map_err(|_| resource("unique_vertices"))?;
    let mut measured_faces = Vec::new();
    measured_faces
        .try_reserve_exact(face_count)
        .map_err(|_| resource("faces"))?;

    for (face_index, face) in exact.faces.iter().enumerate() {
        let boundary = observed
            .face_boundary(face.face)
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        let mut radius = std::array::from_fn(|_| BigRational::zero());
        for (boundary_index, ((vertex, exact_world), expected_vertex)) in
            face.boundary.iter().zip(boundary.vertices()).enumerate()
        {
            if vertex != expected_vertex {
                return Err(MeasuredEnvelopeError::InconsistentPose);
            }
            increment(
                &mut meter.work.boundary_occurrences,
                limits.max_boundary_occurrences,
                "boundary_occurrences",
            )?;
            let source = model
                .vertex_position(*vertex)
                .filter(|point| point.y() == 0.0)
                .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
            let was_present = vertex_registry.contains_key(vertex);
            if was_present {
                increment(
                    &mut meter.work.shared_occurrence_checks,
                    limits.max_shared_occurrence_checks,
                    "shared_occurrence_checks",
                )?;
            } else {
                if vertex_registry.len() >= limits.max_unique_vertices {
                    return Err(resource("unique_vertices"));
                }
                let rest = meter.source_point(source)?;
                vertex_registry.insert(
                    *vertex,
                    VertexWitness {
                        rest,
                        first_face_index: face_index,
                        first_boundary_index: boundary_index,
                    },
                );
            }
            let witness = vertex_registry
                .get(vertex)
                .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
            let first_exact_world =
                &exact.faces[witness.first_face_index].boundary[witness.first_boundary_index].1;
            if first_exact_world != exact_world {
                return Err(MeasuredEnvelopeError::InconsistentPose);
            }
            let recomputed_exact = meter.transform_point(&face.transform, &witness.rest)?;
            if recomputed_exact != *exact_world {
                return Err(MeasuredEnvelopeError::InconsistentPose);
            }
            let binary64_world =
                meter.transform_point(&binary64_transforms[face_index], &witness.rest)?;
            for (component, radius_component) in radius.iter_mut().enumerate() {
                increment(
                    &mut meter.work.point_components,
                    limits.max_point_components,
                    "point_components",
                )?;
                let delta = meter.difference(
                    &binary64_world.coordinates[component],
                    &exact_world.coordinates[component],
                )?;
                if delta > *radius_component {
                    *radius_component = delta;
                }
            }
        }
        // Charge retained output before storing the face. A limit error cannot
        // leave even an internal vector containing uncharged radii.
        for value in &radius {
            meter.charge_output(value)?;
        }
        measured_faces.push(MeasuredFaceEnvelope {
            face: face.face,
            depth: depths[face_index],
            radius,
        });
    }
    meter.work.unique_vertices = vertex_registry.len();
    if meter.work.unique_vertices > limits.max_unique_vertices {
        return Err(resource("unique_vertices"));
    }
    if meter.work.unique_vertices != exact.work.unique_vertices {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }

    authenticate_hinge_paths(
        exact,
        observed,
        &binary64_transforms,
        &vertex_registry,
        &measured_faces,
        &mut meter,
    )?;

    if meter.work.transform_scalars != transform_scalars
        || meter.work.boundary_occurrences != boundary_occurrences
        || meter.work.point_components != point_components
        || meter.work.hinge_feature_points != hinge_feature_points
        || meter.work.exact_hinge_path_checks != exact_hinge_path_checks
        || meter.work.binary64_hinge_path_checks != binary64_hinge_path_checks
        || meter.work.hinge_component_checks != hinge_component_checks
        || meter.work.hinge_transform_checks != hinge_count
        || meter.work.certificate_reads != hinge_count
        || meter.work.exact_point_transforms != expected_point_transforms
        || meter.work.input_scalars != expected_input_scalars
    {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }

    meter.work.exact = meter.exact.work.clone();
    Ok(MeasuredBinary64AffineEnvelope {
        bound: observed,
        faces: measured_faces,
        work: meter.work,
    })
}

fn authenticate_tree_and_depths(
    exact: &RationalCayleyTreePose<'_>,
    observed: BoundMaterialTreePose<'_>,
    meter: &mut EnvelopeMeter<'_>,
) -> Result<Vec<usize>, MeasuredEnvelopeError> {
    let face_count = exact.faces.len();
    let mut children = Vec::<Vec<usize>>::new();
    children
        .try_reserve_exact(face_count)
        .map_err(|_| resource("faces"))?;
    children.resize_with(face_count, Vec::new);
    let mut indegree = vec![0_usize; face_count];
    let model_hinges = observed.model().hinges();
    let angles = observed.pose().hinge_angles();

    for (index, hinge) in exact.hinges.iter().enumerate() {
        let model_hinge = model_hinges
            .get(index)
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        let angle = angles
            .get(index)
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        let base_rotation_sign = match model_hinge.assignment() {
            FoldAssignment::Mountain => 1_i8,
            FoldAssignment::Valley => -1_i8,
        };
        let expected_rotation_sign = if hinge.parent == model_hinge.left_face()
            && hinge.child == model_hinge.right_face()
        {
            base_rotation_sign
        } else if hinge.parent == model_hinge.right_face() && hinge.child == model_hinge.left_face()
        {
            -base_rotation_sign
        } else {
            return Err(MeasuredEnvelopeError::InconsistentPose);
        };
        if hinge.edge != model_hinge.edge()
            || hinge.edge != angle.edge()
            || hinge.angle_magnitude_bits != angle.angle_degrees().to_bits()
            || hinge.rotation_sign != expected_rotation_sign
            || hinge.parent == hinge.child
            || !same_hinge_faces(
                hinge.parent,
                hinge.child,
                model_hinge.left_face(),
                model_hinge.right_face(),
            )
        {
            return Err(MeasuredEnvelopeError::InconsistentPose);
        }
        increment(
            &mut meter.work.certificate_reads,
            meter.limits.max_certificate_reads,
            "certificate_reads",
        )?;
        if !certificate_is_consistent(
            &hinge.certificate,
            hinge.rotation_sign,
            angle.angle_degrees(),
            meter,
        )? {
            return Err(MeasuredEnvelopeError::InconsistentPose);
        }
        let parent = exact_face_index(&exact.faces, hinge.parent)?;
        let child = exact_face_index(&exact.faces, hinge.child)?;
        children[parent]
            .try_reserve(1)
            .map_err(|_| resource("adjacency_entries"))?;
        children[parent].push(child);
        indegree[child] = indegree[child]
            .checked_add(1)
            .ok_or_else(|| resource("adjacency_entries"))?;
    }

    let root_face = if exact.hinges.is_empty() {
        exact
            .faces
            .first()
            .map(|face| face.face)
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?
    } else {
        exact
            .fixed_face
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?
    };
    let root = exact_face_index(&exact.faces, root_face)?;
    if indegree[root] != 0
        || indegree
            .iter()
            .enumerate()
            .any(|(index, degree)| index != root && *degree != 1)
    {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }

    let mut depths = vec![usize::MAX; face_count];
    depths[root] = 0;
    let mut queue = VecDeque::new();
    queue
        .try_reserve(face_count)
        .map_err(|_| resource("faces"))?;
    queue.push_back(root);
    let mut visited = 0_usize;
    while let Some(parent) = queue.pop_front() {
        visited = visited.checked_add(1).ok_or_else(|| resource("faces"))?;
        let parent_depth = depths[parent];
        for child in &children[parent] {
            if depths[*child] != usize::MAX {
                return Err(MeasuredEnvelopeError::InconsistentPose);
            }
            let depth = parent_depth
                .checked_add(1)
                .ok_or_else(|| resource("depth"))?;
            check_count(depth, meter.limits.max_depth, "depth")?;
            add_bounded(
                &mut meter.work.total_depth,
                depth,
                meter.limits.max_total_depth,
                "total_depth",
            )?;
            meter.work.max_depth = meter.work.max_depth.max(depth);
            depths[*child] = depth;
            queue.push_back(*child);
        }
    }
    if visited != face_count {
        return Err(MeasuredEnvelopeError::InconsistentPose);
    }
    Ok(depths)
}

fn authenticate_hinge_paths(
    exact: &RationalCayleyTreePose<'_>,
    observed: BoundMaterialTreePose<'_>,
    binary64_transforms: &[ExactRigidTransform],
    vertex_registry: &HashMap<VertexId, VertexWitness>,
    faces: &[MeasuredFaceEnvelope],
    meter: &mut EnvelopeMeter<'_>,
) -> Result<(), MeasuredEnvelopeError> {
    for (index, hinge) in exact.hinges.iter().enumerate() {
        let model_hinge = &observed.model().hinges()[index];
        let parent_index = exact_face_index(&exact.faces, hinge.parent)?;
        let child_index = exact_face_index(&exact.faces, hinge.child)?;
        let parent_binary64 = observed
            .pose()
            .face_transform(hinge.parent)
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        let hinge_parent_binary64 = observed
            .pose()
            .hinge_parent_transform(hinge.edge)
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        increment(
            &mut meter.work.hinge_transform_checks,
            meter.limits.max_hinge_transform_checks,
            "hinge_transform_checks",
        )?;
        if !same_transform_bits(parent_binary64, hinge_parent_binary64) {
            return Err(MeasuredEnvelopeError::InconsistentPose);
        }
        // The raw 12 coefficients were compared bit-for-bit above, so the
        // already lifted parent transform is also the exact binary64-affine
        // hinge-parent path. Reusing it avoids a second retained allocation.

        let start = vertex_registry
            .get(&hinge.endpoint_vertices[0])
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        let end = vertex_registry
            .get(&hinge.endpoint_vertices[1])
            .ok_or(MeasuredEnvelopeError::InconsistentPose)?;
        let start_exact_world = witness_exact_world(exact, start)?;
        let end_exact_world = witness_exact_world(exact, end)?;
        if observed.model().vertex_position(hinge.endpoint_vertices[0]) != Some(model_hinge.start())
            || observed.model().vertex_position(hinge.endpoint_vertices[1])
                != Some(model_hinge.end())
            || start_exact_world != &hinge.world_endpoints[0]
            || end_exact_world != &hinge.world_endpoints[1]
        {
            return Err(MeasuredEnvelopeError::InconsistentPose);
        }
        let rest_midpoint = meter.midpoint(&start.rest, &end.rest)?;
        let world_midpoint =
            meter.midpoint(&hinge.world_endpoints[0], &hinge.world_endpoints[1])?;
        let features = [
            (&start.rest, &hinge.world_endpoints[0]),
            (&end.rest, &hinge.world_endpoints[1]),
            (&rest_midpoint, &world_midpoint),
        ];

        for (rest, expected) in features {
            increment(
                &mut meter.work.hinge_feature_points,
                meter.limits.max_hinge_feature_points,
                "hinge_feature_points",
            )?;
            let exact_parent = meter.transform_point(&exact.faces[parent_index].transform, rest)?;
            increment(
                &mut meter.work.exact_hinge_path_checks,
                meter.limits.max_exact_hinge_path_checks,
                "exact_hinge_path_checks",
            )?;
            let exact_child = meter.transform_point(&exact.faces[child_index].transform, rest)?;
            increment(
                &mut meter.work.exact_hinge_path_checks,
                meter.limits.max_exact_hinge_path_checks,
                "exact_hinge_path_checks",
            )?;
            increment(
                &mut meter.work.exact_hinge_path_checks,
                meter.limits.max_exact_hinge_path_checks,
                "exact_hinge_path_checks",
            )?;
            if exact_parent != *expected || exact_child != *expected {
                return Err(MeasuredEnvelopeError::InconsistentPose);
            }

            let binary64_parent =
                meter.transform_point(&binary64_transforms[parent_index], rest)?;
            increment(
                &mut meter.work.binary64_hinge_path_checks,
                meter.limits.max_binary64_hinge_path_checks,
                "binary64_hinge_path_checks",
            )?;
            let binary64_child = meter.transform_point(&binary64_transforms[child_index], rest)?;
            increment(
                &mut meter.work.binary64_hinge_path_checks,
                meter.limits.max_binary64_hinge_path_checks,
                "binary64_hinge_path_checks",
            )?;
            let binary64_hinge = meter.transform_point(&binary64_transforms[parent_index], rest)?;
            increment(
                &mut meter.work.binary64_hinge_path_checks,
                meter.limits.max_binary64_hinge_path_checks,
                "binary64_hinge_path_checks",
            )?;

            for (path, radius) in [
                (&binary64_parent, &faces[parent_index].radius),
                (&binary64_child, &faces[child_index].radius),
                (&binary64_hinge, &faces[parent_index].radius),
            ] {
                for (component, radius_component) in radius.iter().enumerate() {
                    increment(
                        &mut meter.work.hinge_component_checks,
                        meter.limits.max_hinge_component_checks,
                        "hinge_component_checks",
                    )?;
                    let delta = meter.difference(
                        &path.coordinates[component],
                        &expected.coordinates[component],
                    )?;
                    if delta > *radius_component {
                        return Err(MeasuredEnvelopeError::InconsistentPose);
                    }
                }
            }
        }
    }
    Ok(())
}

fn witness_exact_world<'a>(
    exact: &'a RationalCayleyTreePose<'_>,
    witness: &VertexWitness,
) -> Result<&'a ExactPoint3, MeasuredEnvelopeError> {
    exact
        .faces
        .get(witness.first_face_index)
        .and_then(|face| face.boundary.get(witness.first_boundary_index))
        .map(|(_, point)| point)
        .ok_or(MeasuredEnvelopeError::InconsistentPose)
}

fn exact_face_index(faces: &[ExactFacePose], face: FaceId) -> Result<usize, MeasuredEnvelopeError> {
    faces
        .binary_search_by_key(&face.canonical_bytes(), |candidate| {
            candidate.face.canonical_bytes()
        })
        .map_err(|_| MeasuredEnvelopeError::InconsistentPose)
}

fn certificate_is_consistent(
    certificate: &ExactAngleCertificate,
    rotation_sign: i8,
    angle_magnitude_degrees: f64,
    meter: &mut EnvelopeMeter<'_>,
) -> Result<bool, MeasuredEnvelopeError> {
    if !matches!(rotation_sign, -1 | 1) {
        return Ok(false);
    }
    let magnitude = meter.input_scalar(angle_magnitude_degrees)?;
    let expected_target = if rotation_sign < 0 {
        -magnitude
    } else {
        magnitude
    };
    Ok(match certificate {
        ExactAngleCertificate::Exact { target_degrees } => target_degrees == &expected_target,
        ExactAngleCertificate::Bounded(certificate) => {
            certificate.target_degrees == expected_target
                && certificate.max_error_radians >= BigRational::zero()
                && certificate.max_error_degrees >= BigRational::zero()
                && certificate.max_error_degrees < certificate.acceptance_degrees
        }
    })
}

fn same_hinge_faces(parent: FaceId, child: FaceId, left: FaceId, right: FaceId) -> bool {
    (parent == left && child == right) || (parent == right && child == left)
}

fn same_transform_bits(left: RigidTransform, right: RigidTransform) -> bool {
    let left_rows = left.rotation_rows();
    let right_rows = right.rotation_rows();
    left_rows
        .iter()
        .flatten()
        .zip(right_rows.iter().flatten())
        .all(|(left, right)| left.to_bits() == right.to_bits())
        && [
            left.translation().x(),
            left.translation().y(),
            left.translation().z(),
        ]
        .into_iter()
        .zip([
            right.translation().x(),
            right.translation().y(),
            right.translation().z(),
        ])
        .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn map_exact<T>(result: Result<T, CayleyError>) -> Result<T, MeasuredEnvelopeError> {
    result.map_err(|error| match error {
        CayleyError::ResourceLimitExceeded {
            resource: resource_name,
            ..
        } => resource(resource_name),
        _ => MeasuredEnvelopeError::InconsistentPose,
    })
}

fn rational_storage_bits(
    value: &BigRational,
    resource_name: &'static str,
) -> Result<usize, MeasuredEnvelopeError> {
    bigint_bits(value.numer())
        .checked_add(bigint_bits(value.denom()))
        .ok_or_else(|| resource(resource_name))
}

fn increment(
    current: &mut usize,
    maximum: usize,
    resource_name: &'static str,
) -> Result<(), MeasuredEnvelopeError> {
    add_bounded(current, 1, maximum, resource_name)
}

fn add_bounded(
    current: &mut usize,
    amount: usize,
    maximum: usize,
    resource_name: &'static str,
) -> Result<(), MeasuredEnvelopeError> {
    let next = current
        .checked_add(amount)
        .ok_or_else(|| resource(resource_name))?;
    if next > maximum {
        return Err(resource(resource_name));
    }
    *current = next;
    Ok(())
}

fn checked_product(
    count: usize,
    factor: usize,
    resource_name: &'static str,
) -> Result<usize, MeasuredEnvelopeError> {
    count
        .checked_mul(factor)
        .ok_or_else(|| resource(resource_name))
}

fn check_count(
    count: usize,
    maximum: usize,
    resource_name: &'static str,
) -> Result<(), MeasuredEnvelopeError> {
    if count > maximum {
        Err(resource(resource_name))
    } else {
        Ok(())
    }
}

const fn resource(resource: &'static str) -> MeasuredEnvelopeError {
    MeasuredEnvelopeError::ResourceLimitExceeded { resource }
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
    use ori_kinematics::{
        CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
        TreeKinematicsLimits,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use super::*;

    fn test_vertex_id(index: u64) -> VertexId {
        serde_json::from_str(&format!("\"00000000-0000-4000-8100-{index:012x}\""))
            .expect("fixed vertex id")
    }

    fn test_edge_id(index: u64) -> EdgeId {
        serde_json::from_str(&format!("\"00000000-0000-4000-9100-{index:012x}\""))
            .expect("fixed edge id")
    }

    fn test_project_id() -> ProjectId {
        serde_json::from_str("\"00000000-0000-4000-b100-0000000000c1\"").expect("fixed project id")
    }

    fn test_vertex(index: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: test_vertex_id(index),
            position: Point2::new(x, y),
        }
    }

    fn test_edge(index: u64, start: VertexId, end: VertexId, kind: EdgeKind) -> Edge {
        Edge {
            id: test_edge_id(index),
            start,
            end,
            kind,
        }
    }

    fn prepare_model(
        vertices: Vec<Vertex>,
        mut edges: Vec<Edge>,
        boundary: Vec<VertexId>,
        hinge: Option<(u64, VertexId, VertexId)>,
        source_revision: u64,
    ) -> MaterialTreeKinematicsModel {
        if let Some((index, start, end)) = hinge {
            edges.push(test_edge(index, start, end, EdgeKind::Mountain));
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: test_project_id(),
            source_revision,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("test topology"),
            TreeKinematicsLimits::default(),
        )
        .expect("material test model")
    }

    fn single_face_model() -> MaterialTreeKinematicsModel {
        let vertices = vec![
            test_vertex(21, 0.0, 0.0),
            test_vertex(22, 8.0, 0.0),
            test_vertex(23, 8.0, 6.0),
            test_vertex(24, 0.0, 6.0),
        ];
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let edges = (0..boundary.len())
            .map(|index| {
                test_edge(
                    index as u64 + 21,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect();
        prepare_model(vertices, edges, boundary, None, 2_001)
    }

    /// Convex A-B-C-D-E with the 3-4-5 material-space hinge A-D.
    ///
    /// The split yields one triangle and one quadrilateral: F=2, H=1,
    /// boundary occurrences=7, unique vertices=5.
    fn one_hinge_model() -> MaterialTreeKinematicsModel {
        let vertices = vec![
            test_vertex(1, 0.0, 0.0),
            test_vertex(2, 4.0, -1.0),
            test_vertex(3, 7.0, 2.0),
            test_vertex(4, 3.0, 4.0),
            test_vertex(5, -1.0, 3.0),
        ];
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let edges = (0..boundary.len())
            .map(|index| {
                test_edge(
                    index as u64 + 1,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect();
        prepare_model(
            vertices,
            edges,
            boundary.clone(),
            Some((6, boundary[0], boundary[3])),
            2_002,
        )
    }

    fn one_hinge_angles(model: &MaterialTreeKinematicsModel, degrees: f64) -> CanonicalHingeAngles {
        CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), degrees).expect("valid test angle"))
                .collect(),
        )
        .expect("canonical one-hinge angles")
    }

    fn one_hinge_pose(
        model: &MaterialTreeKinematicsModel,
        root: FaceId,
        degrees: f64,
    ) -> MaterialTreePose {
        model
            .solve(Some(root), &one_hinge_angles(model, degrees))
            .expect("one-hinge pose")
    }

    fn exact_pose<'a>(
        model: &'a MaterialTreeKinematicsModel,
        pose: &'a MaterialTreePose,
    ) -> RationalCayleyTreePose<'a> {
        prepare_rational_cayley_tree_pose_v1(
            model.bind_pose(pose).expect("bound material pose"),
            ExactTreePoseLimits::default(),
        )
        .expect("exact Cayley test pose")
    }

    fn independent_exact_f64(value: f64) -> BigRational {
        assert!(value.is_finite());
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        if exponent_bits == 0 && fraction == 0 {
            return BigRational::zero();
        }
        let (significand, exponent) = if exponent_bits == 0 {
            (fraction, -1_074)
        } else {
            ((1_u64 << 52) | fraction, exponent_bits - 1_023 - 52)
        };
        let mut numerator = BigInt::from(significand);
        if negative {
            numerator = -numerator;
        }
        if exponent >= 0 {
            BigRational::from_integer(numerator << exponent as usize)
        } else {
            BigRational::new(numerator, BigInt::one() << (-exponent) as usize)
        }
    }

    fn independent_source(point: Point3) -> ExactPoint3 {
        ExactPoint3 {
            coordinates: [
                independent_exact_f64(point.x()),
                independent_exact_f64(point.y()),
                independent_exact_f64(point.z()),
            ],
        }
    }

    fn independent_binary64_apply(transform: RigidTransform, point: &ExactPoint3) -> ExactPoint3 {
        let rows = transform.rotation_rows();
        let translation = transform.translation();
        let translation = [translation.x(), translation.y(), translation.z()];
        ExactPoint3 {
            coordinates: std::array::from_fn(|row| {
                let mut value = independent_exact_f64(translation[row]);
                for (coefficient, coordinate) in rows[row].iter().zip(&point.coordinates) {
                    value += independent_exact_f64(*coefficient) * coordinate;
                }
                value
            }),
        }
    }

    fn independent_exact_apply(
        transform: &ExactRigidTransform,
        point: &ExactPoint3,
    ) -> ExactPoint3 {
        ExactPoint3 {
            coordinates: std::array::from_fn(|row| {
                let mut value = transform.translation.coordinates[row].clone();
                for (coefficient, coordinate) in
                    transform.rotation[row].iter().zip(&point.coordinates)
                {
                    value += coefficient * coordinate;
                }
                value
            }),
        }
    }

    fn independent_midpoint(first: &ExactPoint3, second: &ExactPoint3) -> ExactPoint3 {
        let two = BigRational::from_integer(BigInt::from(2_u8));
        ExactPoint3 {
            coordinates: std::array::from_fn(|component| {
                (&first.coordinates[component] + &second.coordinates[component]) / &two
            }),
        }
    }

    fn independent_delta(left: &ExactPoint3, right: &ExactPoint3) -> [BigRational; 3] {
        std::array::from_fn(|component| {
            (&left.coordinates[component] - &right.coordinates[component]).abs()
        })
    }

    fn measured_face<'a>(
        measured: &'a MeasuredBinary64AffineEnvelope<'_>,
        face: FaceId,
    ) -> &'a MeasuredFaceEnvelope {
        measured
            .faces
            .binary_search_by_key(&face.canonical_bytes(), |candidate| {
                candidate.face.canonical_bytes()
            })
            .ok()
            .and_then(|index| measured.faces.get(index))
            .expect("measured face")
    }

    fn exact_face_index_for_test(exact: &RationalCayleyTreePose<'_>, face: FaceId) -> usize {
        exact
            .faces
            .binary_search_by_key(&face.canonical_bytes(), |candidate| {
                candidate.face.canonical_bytes()
            })
            .expect("exact face index")
    }

    #[test]
    fn single_face_identity_has_bit_exact_zero_radius() {
        let model = single_face_model();
        let angles = CanonicalHingeAngles::new(Vec::new()).expect("empty angles");
        let pose = model.solve(None, &angles).expect("single-face pose");
        let bound = model.bind_pose(&pose).expect("bound single-face pose");
        let exact = exact_pose(&model, &pose);
        let measured =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("identity measurement");

        assert!(measured.is_for(bound));
        assert_eq!(measured.faces.len(), 1);
        assert_eq!(measured.faces[0].depth, 0);
        assert_eq!(
            measured.faces[0].radius,
            std::array::from_fn(|_| BigRational::zero())
        );
        assert_eq!(measured.work.faces, 1);
        assert_eq!(measured.work.hinges, 0);
        assert_eq!(measured.work.adjacency_entries, 0);
        assert_eq!(measured.work.max_depth, 0);
        assert_eq!(measured.work.total_depth, 0);
        assert_eq!(measured.work.transform_scalars, 12);
        assert_eq!(measured.work.boundary_occurrences, 4);
        assert_eq!(measured.work.point_components, 12);
        assert_eq!(measured.work.unique_vertices, 4);
        assert_eq!(measured.work.shared_occurrence_checks, 0);
        assert_eq!(measured.work.hinge_feature_points, 0);
        assert_eq!(measured.work.exact_hinge_path_checks, 0);
        assert_eq!(measured.work.binary64_hinge_path_checks, 0);
        assert_eq!(measured.work.hinge_component_checks, 0);
        assert_eq!(measured.work.hinge_transform_checks, 0);
        assert_eq!(measured.work.certificate_reads, 0);
        assert_eq!(measured.work.exact_point_transforms, 8);
        assert_eq!(measured.work.input_scalars, 24);
    }

    #[test]
    fn measured_one_hinge_37_covers_every_occurrence_and_all_hinge_paths() {
        let model = one_hinge_model();
        let root = model.hinges()[0].left_face();
        let pose = one_hinge_pose(&model, root, 37.0);
        let bound = model.bind_pose(&pose).expect("bound one-hinge pose");
        let exact = exact_pose(&model, &pose);
        let first =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("first measurement");
        let second =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("second measurement");

        assert_eq!(first.faces, second.faces);
        assert_eq!(first.work, second.work);
        assert_eq!(
            first.faces.iter().map(|face| face.face).collect::<Vec<_>>(),
            exact.faces.iter().map(|face| face.face).collect::<Vec<_>>()
        );

        let mut occurrence_count = 0_usize;
        for face in &exact.faces {
            let transform = pose.face_transform(face.face).expect("face transform");
            let mut expected_radius = std::array::from_fn(|_| BigRational::zero());
            for (vertex, exact_world) in &face.boundary {
                occurrence_count += 1;
                let source = independent_source(
                    model
                        .vertex_position(*vertex)
                        .expect("material source vertex"),
                );
                let binary64_world = independent_binary64_apply(transform, &source);
                let delta = independent_delta(&binary64_world, exact_world);
                for component in 0..3 {
                    if delta[component] > expected_radius[component] {
                        expected_radius[component] = delta[component].clone();
                    }
                }
            }
            assert_eq!(measured_face(&first, face.face).radius, expected_radius);
        }
        assert_eq!(occurrence_count, 7);
        assert_eq!(
            measured_face(&first, root).radius,
            std::array::from_fn(|_| BigRational::zero())
        );
        assert!(
            first
                .faces
                .iter()
                .filter(|face| face.face != root)
                .flat_map(|face| &face.radius)
                .any(|value| !value.is_zero())
        );

        let mut hinge_path_mask = 0_u16;
        for hinge in &exact.hinges {
            let parent_index = exact_face_index_for_test(&exact, hinge.parent);
            let child_index = exact_face_index_for_test(&exact, hinge.child);
            let parent_transform = pose.face_transform(hinge.parent).expect("parent transform");
            let hinge_parent_transform = pose
                .hinge_parent_transform(hinge.edge)
                .expect("hinge-parent transform");
            assert!(same_transform_bits(
                parent_transform,
                hinge_parent_transform
            ));

            let rest_start = independent_source(
                model
                    .vertex_position(hinge.endpoint_vertices[0])
                    .expect("hinge start"),
            );
            let rest_end = independent_source(
                model
                    .vertex_position(hinge.endpoint_vertices[1])
                    .expect("hinge end"),
            );
            let rest_midpoint = independent_midpoint(&rest_start, &rest_end);
            let world_midpoint =
                independent_midpoint(&hinge.world_endpoints[0], &hinge.world_endpoints[1]);
            let features = [
                (&rest_start, &hinge.world_endpoints[0]),
                (&rest_end, &hinge.world_endpoints[1]),
                (&rest_midpoint, &world_midpoint),
            ];
            for (feature_index, (rest, expected)) in features.into_iter().enumerate() {
                assert_eq!(
                    independent_exact_apply(&exact.faces[parent_index].transform, rest),
                    *expected
                );
                assert_eq!(
                    independent_exact_apply(&exact.faces[child_index].transform, rest),
                    *expected
                );
                let paths = [
                    (
                        independent_binary64_apply(parent_transform, rest),
                        measured_face(&first, hinge.parent),
                    ),
                    (
                        independent_binary64_apply(
                            pose.face_transform(hinge.child).expect("child transform"),
                            rest,
                        ),
                        measured_face(&first, hinge.child),
                    ),
                    (
                        independent_binary64_apply(hinge_parent_transform, rest),
                        measured_face(&first, hinge.parent),
                    ),
                ];
                for (path_index, (path, face)) in paths.into_iter().enumerate() {
                    let delta = independent_delta(&path, expected);
                    assert!(
                        delta
                            .iter()
                            .zip(&face.radius)
                            .all(|(delta, radius)| delta <= radius)
                    );
                    hinge_path_mask |= 1 << (feature_index * 3 + path_index);
                }
            }
        }
        assert_eq!(hinge_path_mask, 0x01ff);

        assert_eq!(first.work.faces, 2);
        assert_eq!(first.work.hinges, 1);
        assert_eq!(first.work.adjacency_entries, 1);
        assert_eq!(first.work.max_depth, 1);
        assert_eq!(first.work.total_depth, 1);
        assert_eq!(first.work.transform_scalars, 24);
        assert_eq!(first.work.boundary_occurrences, 7);
        assert_eq!(first.work.point_components, 21);
        assert_eq!(first.work.unique_vertices, 5);
        assert_eq!(first.work.shared_occurrence_checks, 2);
        assert_eq!(first.work.hinge_feature_points, 3);
        assert_eq!(first.work.exact_hinge_path_checks, 9);
        assert_eq!(first.work.binary64_hinge_path_checks, 9);
        assert_eq!(first.work.hinge_component_checks, 27);
        assert_eq!(first.work.hinge_transform_checks, 1);
        assert_eq!(first.work.certificate_reads, 1);
        assert_eq!(first.work.exact_point_transforms, 29);
        assert_eq!(first.work.input_scalars, 40);
    }

    #[test]
    fn authority_accepts_only_the_same_issued_pose_instance() {
        let model = one_hinge_model();
        let root = model.hinges()[0].left_face();
        let other_root = model.hinges()[0].right_face();
        let pose = one_hinge_pose(&model, root, 37.0);
        let pose_clone = pose.clone();
        let model_clone = model.clone();
        let bound = model.bind_pose(&pose).expect("original bound");
        let clone_bound = model_clone
            .bind_pose(&pose_clone)
            .expect("clone-issued bound");
        let exact = exact_pose(&model, &pose);

        let measured = measure_binary64_affine_envelope(
            &exact,
            clone_bound,
            MeasuredEnvelopeLimits::default(),
        )
        .expect("clone must preserve authority");
        assert!(measured.is_for(bound));
        assert!(measured.is_for(clone_bound));

        let aba_pose = one_hinge_pose(&model, root, 37.0);
        let aba_bound = model.bind_pose(&aba_pose).expect("ABA bound");
        assert_eq!(
            measure_binary64_affine_envelope(&exact, aba_bound, MeasuredEnvelopeLimits::default())
                .unwrap_err(),
            MeasuredEnvelopeError::AuthorityMismatch
        );
        assert!(!measured.is_for(aba_bound));

        let foreign_model = one_hinge_model();
        let foreign_pose = one_hinge_pose(&foreign_model, root, 37.0);
        let foreign_bound = foreign_model
            .bind_pose(&foreign_pose)
            .expect("foreign bound");
        assert_eq!(
            measure_binary64_affine_envelope(
                &exact,
                foreign_bound,
                MeasuredEnvelopeLimits::default()
            )
            .unwrap_err(),
            MeasuredEnvelopeError::AuthorityMismatch
        );

        let rerooted_pose = one_hinge_pose(&model, other_root, 37.0);
        let rerooted_bound = model.bind_pose(&rerooted_pose).expect("rerooted bound");
        assert_eq!(
            measure_binary64_affine_envelope(
                &exact,
                rerooted_bound,
                MeasuredEnvelopeLimits::default()
            )
            .unwrap_err(),
            MeasuredEnvelopeError::AuthorityMismatch
        );
        let rerooted_exact = exact_pose(&model, &rerooted_pose);
        let rerooted_measured = measure_binary64_affine_envelope(
            &rerooted_exact,
            rerooted_bound,
            MeasuredEnvelopeLimits::default(),
        )
        .expect("rerooted self-authority");
        assert!(rerooted_measured.is_for(rerooted_bound));

        let next_angle = f64::from_bits(37.0_f64.to_bits() + 1);
        let next_pose = one_hinge_pose(&model, root, next_angle);
        let next_bound = model.bind_pose(&next_pose).expect("one-ULP bound");
        assert_eq!(
            measure_binary64_affine_envelope(&exact, next_bound, MeasuredEnvelopeLimits::default())
                .unwrap_err(),
            MeasuredEnvelopeError::AuthorityMismatch
        );
        let next_exact = exact_pose(&model, &next_pose);
        let next_measured = measure_binary64_affine_envelope(
            &next_exact,
            next_bound,
            MeasuredEnvelopeLimits::default(),
        )
        .expect("one-ULP self-authority");
        assert!(next_measured.is_for(next_bound));
    }

    fn limits_tight_to(work: &MeasuredEnvelopeWork) -> MeasuredEnvelopeLimits {
        MeasuredEnvelopeLimits {
            max_faces: work.faces,
            max_hinges: work.hinges,
            max_adjacency_entries: work.adjacency_entries,
            max_depth: work.max_depth,
            max_total_depth: work.total_depth,
            max_transform_scalars: work.transform_scalars,
            max_boundary_occurrences: work.boundary_occurrences,
            max_point_components: work.point_components,
            max_unique_vertices: work.unique_vertices,
            max_shared_occurrence_checks: work.shared_occurrence_checks,
            max_hinge_feature_points: work.hinge_feature_points,
            max_exact_hinge_path_checks: work.exact_hinge_path_checks,
            max_binary64_hinge_path_checks: work.binary64_hinge_path_checks,
            max_hinge_component_checks: work.hinge_component_checks,
            max_hinge_transform_checks: work.hinge_transform_checks,
            max_certificate_reads: work.certificate_reads,
            max_exact_point_transforms: work.exact_point_transforms,
            max_input_scalars: work.input_scalars,
            max_input_bits: work.max_input_bits,
            max_total_input_bits: work.total_input_bits,
            max_output_bits: work.max_output_bits,
            max_total_output_bits: work.total_output_bits,
            exact: MeasuredEnvelopeLimits::default().exact,
        }
    }

    fn assert_resource_error(
        result: Result<MeasuredBinary64AffineEnvelope<'_>, MeasuredEnvelopeError>,
        expected: &'static str,
    ) {
        assert_eq!(
            result.unwrap_err(),
            MeasuredEnvelopeError::ResourceLimitExceeded { resource: expected }
        );
    }

    #[test]
    fn every_structural_limit_accepts_exact_work_and_rejects_one_short_atomically() {
        let model = one_hinge_model();
        let root = model.hinges()[0].left_face();
        let pose = one_hinge_pose(&model, root, 37.0);
        let bound = model.bind_pose(&pose).expect("bound one-hinge pose");
        let exact = exact_pose(&model, &pose);
        let baseline =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("baseline measurement");
        let tight = limits_tight_to(&baseline.work);
        let tight_result = measure_binary64_affine_envelope(&exact, bound, tight)
            .expect("all structural limits at exact work");
        assert_eq!(tight_result.faces, baseline.faces);
        assert_eq!(tight_result.work, baseline.work);

        macro_rules! one_short {
            ($field:ident, $resource:literal) => {{
                let mut limits = tight;
                assert!(limits.$field > 0, "{} must be exercised", $resource);
                limits.$field -= 1;
                assert_resource_error(
                    measure_binary64_affine_envelope(&exact, bound, limits),
                    $resource,
                );
            }};
        }

        one_short!(max_faces, "faces");
        one_short!(max_hinges, "hinges");
        one_short!(max_adjacency_entries, "adjacency_entries");
        one_short!(max_depth, "depth");
        one_short!(max_total_depth, "total_depth");
        one_short!(max_transform_scalars, "transform_scalars");
        one_short!(max_boundary_occurrences, "boundary_occurrences");
        one_short!(max_point_components, "point_components");
        one_short!(max_unique_vertices, "unique_vertices");
        one_short!(max_shared_occurrence_checks, "shared_occurrence_checks");
        one_short!(max_hinge_feature_points, "hinge_feature_points");
        one_short!(max_exact_hinge_path_checks, "exact_hinge_path_checks");
        one_short!(max_binary64_hinge_path_checks, "binary64_hinge_path_checks");
        one_short!(max_hinge_component_checks, "hinge_component_checks");
        one_short!(max_hinge_transform_checks, "hinge_transform_checks");
        one_short!(max_certificate_reads, "certificate_reads");
        one_short!(max_exact_point_transforms, "exact_point_transforms");
        one_short!(max_input_scalars, "input_scalars");
        one_short!(max_input_bits, "input_bits");
        one_short!(max_total_input_bits, "total_input_bits");
        one_short!(max_output_bits, "output_bits");
        one_short!(max_total_output_bits, "total_output_bits");
    }

    #[test]
    fn exact_arithmetic_limits_accept_exact_work_and_reject_one_short() {
        let model = one_hinge_model();
        let root = model.hinges()[0].left_face();
        let pose = one_hinge_pose(&model, root, 37.0);
        let bound = model.bind_pose(&pose).expect("bound one-hinge pose");
        let exact = exact_pose(&model, &pose);
        let baseline =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("baseline measurement");
        let structural = limits_tight_to(&baseline.work);

        let mut interval_limits = structural;
        interval_limits.exact.max_interval_operations = baseline.work.exact.interval_operations;
        let interval_exact = measure_binary64_affine_envelope(&exact, bound, interval_limits)
            .expect("exact interval-operation limit");
        assert_eq!(
            interval_exact.work.exact.interval_operations,
            baseline.work.exact.interval_operations
        );
        interval_limits.exact.max_interval_operations -= 1;
        assert_resource_error(
            measure_binary64_affine_envelope(&exact, bound, interval_limits),
            "interval_operations",
        );

        assert!(baseline.work.exact.max_shift_bits > 0);
        let mut shift_limits = structural;
        shift_limits.exact.max_shift_bits = baseline.work.exact.max_shift_bits;
        let shift_exact = measure_binary64_affine_envelope(&exact, bound, shift_limits)
            .expect("exact shift-bit limit");
        assert_eq!(
            shift_exact.work.exact.max_shift_bits,
            baseline.work.exact.max_shift_bits
        );
        shift_limits.exact.max_shift_bits -= 1;
        assert_resource_error(
            measure_binary64_affine_envelope(&exact, bound, shift_limits),
            "shift_bits",
        );

        let mut lower = 0_usize;
        let mut upper = structural.exact.max_intermediate_bits;
        while lower < upper {
            let candidate = lower + (upper - lower) / 2;
            let mut limits = structural;
            limits.exact.max_intermediate_bits = candidate;
            match measure_binary64_affine_envelope(&exact, bound, limits) {
                Ok(_) => upper = candidate,
                Err(MeasuredEnvelopeError::ResourceLimitExceeded {
                    resource: "intermediate_bits",
                }) => lower = candidate + 1,
                Err(error) => panic!("unexpected minimum-cap error: {error:?}"),
            }
        }
        assert!(lower > 0);
        let mut minimum_limits = structural;
        minimum_limits.exact.max_intermediate_bits = lower;
        let minimum = measure_binary64_affine_envelope(&exact, bound, minimum_limits)
            .expect("minimum accepted intermediate-bit cap");
        assert!(
            minimum
                .work
                .exact
                .max_preflight_bits
                .max(minimum.work.exact.max_observed_bits)
                <= lower
        );
        minimum_limits.exact.max_intermediate_bits -= 1;
        assert_resource_error(
            measure_binary64_affine_envelope(&exact, bound, minimum_limits),
            "intermediate_bits",
        );

        assert!(minimum.work.exact.gcd_fallback_calls > 0);
        assert!(minimum.work.exact.gcd_fallback_input_bits > 0);
        let mut gcd_limits = structural;
        gcd_limits.exact.max_intermediate_bits = lower;
        gcd_limits.exact.max_gcd_fallback_calls = minimum.work.exact.gcd_fallback_calls;
        gcd_limits.exact.max_gcd_fallback_input_bits = minimum.work.exact.gcd_fallback_input_bits;
        let gcd_exact = measure_binary64_affine_envelope(&exact, bound, gcd_limits)
            .expect("exact GCD fallback limits");
        assert_eq!(
            gcd_exact.work.exact.gcd_fallback_calls,
            minimum.work.exact.gcd_fallback_calls
        );
        assert_eq!(
            gcd_exact.work.exact.gcd_fallback_input_bits,
            minimum.work.exact.gcd_fallback_input_bits
        );

        let mut calls_one_short = gcd_limits;
        calls_one_short.exact.max_gcd_fallback_calls -= 1;
        assert_resource_error(
            measure_binary64_affine_envelope(&exact, bound, calls_one_short),
            "gcd_fallback_calls",
        );
        let mut bits_one_short = gcd_limits;
        bits_one_short.exact.max_gcd_fallback_input_bits -= 1;
        assert_resource_error(
            measure_binary64_affine_envelope(&exact, bound, bits_one_short),
            "gcd_fallback_input_bits",
        );
    }

    #[test]
    fn malformed_canonical_inputs_fail_without_a_partial_envelope() {
        let model = one_hinge_model();
        let root = model.hinges()[0].left_face();
        let pose = one_hinge_pose(&model, root, 37.0);
        let bound = model.bind_pose(&pose).expect("bound one-hinge pose");

        let mut reversed_boundary = exact_pose(&model, &pose);
        reversed_boundary.faces[0].boundary.reverse();
        assert_eq!(
            measure_binary64_affine_envelope(
                &reversed_boundary,
                bound,
                MeasuredEnvelopeLimits::default()
            )
            .unwrap_err(),
            MeasuredEnvelopeError::InconsistentPose
        );

        let mut reordered_faces = exact_pose(&model, &pose);
        reordered_faces.faces.swap(0, 1);
        assert_eq!(
            measure_binary64_affine_envelope(
                &reordered_faces,
                bound,
                MeasuredEnvelopeLimits::default()
            )
            .unwrap_err(),
            MeasuredEnvelopeError::InconsistentPose
        );

        let mut wrong_hinge = exact_pose(&model, &pose);
        wrong_hinge.hinges[0].edge = test_edge_id(1);
        assert_eq!(
            measure_binary64_affine_envelope(
                &wrong_hinge,
                bound,
                MeasuredEnvelopeLimits::default()
            )
            .unwrap_err(),
            MeasuredEnvelopeError::InconsistentPose
        );

        let mut wrong_rotation_sign = exact_pose(&model, &pose);
        wrong_rotation_sign.hinges[0].rotation_sign *= -1;
        match &mut wrong_rotation_sign.hinges[0].certificate {
            ExactAngleCertificate::Exact { target_degrees } => {
                *target_degrees = -target_degrees.clone();
            }
            ExactAngleCertificate::Bounded(certificate) => {
                certificate.target_degrees = -certificate.target_degrees.clone();
            }
        }
        assert_eq!(
            measure_binary64_affine_envelope(
                &wrong_rotation_sign,
                bound,
                MeasuredEnvelopeLimits::default()
            )
            .unwrap_err(),
            MeasuredEnvelopeError::InconsistentPose
        );

        let mut wrong_unique_count = exact_pose(&model, &pose);
        wrong_unique_count.work.unique_vertices -= 1;
        assert_eq!(
            measure_binary64_affine_envelope(
                &wrong_unique_count,
                bound,
                MeasuredEnvelopeLimits::default()
            )
            .unwrap_err(),
            MeasuredEnvelopeError::InconsistentPose
        );
    }

    #[test]
    fn checked_counter_overflow_is_atomic() {
        assert_eq!(
            checked_product(usize::MAX, 2, "derived"),
            Err(MeasuredEnvelopeError::ResourceLimitExceeded {
                resource: "derived"
            })
        );
        let mut current = usize::MAX;
        assert_eq!(
            add_bounded(&mut current, 1, usize::MAX, "counter"),
            Err(MeasuredEnvelopeError::ResourceLimitExceeded {
                resource: "counter"
            })
        );
        assert_eq!(current, usize::MAX);
    }
}
