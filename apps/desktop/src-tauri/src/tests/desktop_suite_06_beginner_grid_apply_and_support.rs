#[test]
fn complete_animal_grid_apply_replay_undo_redo_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = vec![
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 8,
            scale_percent: 25,
            spacing_percent: 50,
        },
        25,
        50,
    );
    assert!(ori_domain::animal_complete_bindings_v1(&profile.generation_constraints).is_some());

    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let apply_profile = profile.clone();
    for target in &mut profile.generation_constraints.protrusions {
        target.length_tenths_mm = 270 + u32::from(target.id) * 10;
        target.thickness_tenths_mm = 50 + target.id;
        target.direction_milli[0] = -target.direction_milli[0];
        target.direction_milli[1] = -target.direction_milli[1];
    }
    profile.generation_constraints.protrusions.reverse();
    let temporary = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    assert_eq!(
        temporary
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| target.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    for target in &temporary.generation_constraints.protrusions {
        assert_eq!(
            target.length_tenths_mm,
            ((270 + u32::from(target.id) * 10) * u32::from(point.scale_percent) / 27).max(1)
        );
        assert_eq!(
            target.thickness_tenths_mm,
            ((50 + target.id) * u16::from(point.spacing_percent) / 50).max(1)
        );
    }
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &apply_profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase)
    .unwrap();
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let revision = project.editor.revision();
    let saved_profile = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(apply_profile),
        },
    )
    .unwrap();
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            revision,
            plan.clone(),
        )
        .is_err()
    );
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved_profile.revision,
        plan.clone(),
    )
    .unwrap();
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            saved_profile.revision,
            plan,
        )
        .is_err()
    );
    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    let _redone = execute_redo(&mut project, project_id, undone.revision).unwrap();
    let saved = project.document();
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("complete-animal.ori2"));
    assert_eq!(reopened.document(), saved);
    assert!(
        ori_domain::animal_complete_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints
        )
        .is_some()
    );
    assert!(!reopened.editor.can_undo());
    assert!(!reopened.editor.can_redo());
}

#[test]
fn complete_winged_animal_grid_apply_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = vec![
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
        (ori_domain::BeginnerTargetPartKindV1::Wing, 2),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 10,
            scale_percent: 25,
            spacing_percent: 50,
        },
        25,
        50,
    );
    let binding = ori_domain::animal_complete_winged_bindings_v1(&profile.generation_constraints)
        .expect("strict five-binding winged animal");
    assert_eq!(binding.wing_pair_protrusion_id, 5);
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| {
        plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
    })
    .expect("winged animal grid plan");
    assert_eq!(plan.crease_pattern.vertices.len(), 15);
    assert_eq!(plan.crease_pattern.edges.len(), 14);
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let cancel_generation = ProjectId::new();
    let cancel_work = Arc::new(BeginnerGridWork::default());
    beginner_grid_work()
        .lock()
        .unwrap()
        .insert(cancel_generation, Arc::clone(&cancel_work));
    cancel_beginner_parameter_grid(cancel_generation).unwrap();
    assert!(cancel_work.cancelled.load(Ordering::Acquire));
    beginner_grid_work().lock().unwrap().clear();
    let revision = project.editor.revision();
    let saved_profile = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
    .unwrap();
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved_profile.revision,
        plan.clone(),
    )
    .unwrap();
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            saved_profile.revision,
            plan,
        )
        .is_err()
    );
    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    execute_redo(&mut project, project_id, undone.revision).unwrap();
    let mut saved = project.document();
    saved.thumbnail_svg = None;
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened = ProjectState::from_valid_document(restored, PathBuf::from("winged-animal.ori2"));
    assert_eq!(
        reopened.editor.beginner_design_profile(),
        &saved.beginner_design_profile
    );
    assert!(
        ori_domain::animal_complete_winged_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints,
        )
        .is_some()
    );
}

#[test]
fn symmetry_transforms_are_exact_at_cardinal_angles() {
    assert_eq!(
        mirror_point_left_right(Point2::new(3.0, 4.0), 1.0),
        Point2::new(-1.0, 4.0)
    );
    let center = Point2::new(1.0, 2.0);
    let point = Point2::new(3.0, 4.0);
    for (angle, expected) in [
        (0.0, Point2::new(3.0, 4.0)),
        (90.0, Point2::new(-1.0, 4.0)),
        (180.0, Point2::new(-1.0, 0.0)),
        (270.0, Point2::new(3.0, 0.0)),
    ] {
        let (sin, cos) = symmetry_sin_cos(angle).expect("finite cardinal angle");
        assert_eq!(rotate_point_about(point, center, sin, cos), expected);
    }
    let (sin, cos) = symmetry_sin_cos(37.5).expect("finite non-cardinal angle");
    assert_eq!(sin.to_bits(), 0x3fe3_7af9_3f95_13ea);
    assert_eq!(cos.to_bits(), 0x3fe9_6326_8b57_2492);
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(symmetry_sin_cos(invalid), None);
    }
}

fn execute_command(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    command: Command,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        command,
    )
}

fn execute_undo(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_undo(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

fn execute_redo(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_redo(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

fn execute_edge_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_edge_split(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

fn execute_edge_intersection_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_edge_intersection_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_intersection_cluster_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_intersection_cluster_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        targets,
        junction_vertex_id,
    )
}

fn execute_t_junction_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_t_junction_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_boundary_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_boundary_split(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the Unix epoch")
            .as_nanos();
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "origami2-native-file-tests-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated native-file test directory");
        Self { path }
    }

    #[cfg(target_os = "windows")]
    fn new_relative() -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
        let path = PathBuf::from(format!(
            ".origami2-relative-native-file-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated relative native-file test directory");
        Self { path }
    }

    fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
