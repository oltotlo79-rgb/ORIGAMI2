use super::*;
use std::io::{Cursor, Read};

struct GeometryReferenceFixture {
    project: ProjectState,
    edge: EdgeId,
    target: VertexId,
    deterministic_endpoint: Point2,
}

struct PolarReferenceFixture {
    project: ProjectState,
    start: VertexId,
    target: VertexId,
    start_position: Point2,
    length_mm: f64,
    angle_degrees: f64,
    deterministic_endpoint: Point2,
}

fn edge_reference(edge: EdgeId, property: &str) -> String {
    let wire = serde_json::to_value(edge)
        .expect("edge ID")
        .as_str()
        .expect("wire edge ID")
        .to_owned();
    format!("e.{wire}.{property}")
}

fn vertex_id_ending_in_e() -> VertexId {
    serde_json::from_str("\"12345678-1234-4234-8234-123456789abe\"")
        .expect("fixed vertex ID ending in e")
}

fn deterministic_edge_geometry(delta_x: f64, delta_y: f64) -> Point2 {
    let length =
        ori_numeric::deterministic_hypot_v1(delta_x, delta_y).expect("deterministic length");
    let angle = ori_numeric::deterministic_atan2_v1(delta_y, delta_x)
        .and_then(ori_numeric::deterministic_radians_to_degrees_v1)
        .expect("deterministic angle")
        .rem_euclid(360.0);
    Point2::new(length, if angle == 0.0 { 0.0 } else { angle })
}

fn ori2_required_features(bytes: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("ORI2 ZIP");
    let mut manifest = String::new();
    archive
        .by_name(ori_formats::ORI2_MANIFEST_PATH)
        .expect("ORI2 manifest")
        .read_to_string(&mut manifest)
        .expect("read ORI2 manifest");
    serde_json::from_str::<ori_formats::Ori2Manifest>(&manifest)
        .expect("parse ORI2 manifest")
        .required_features
}

fn geometry_reference_fixture(target_position: Point2) -> GeometryReferenceFixture {
    let start = VertexId::new();
    let end = VertexId::new();
    let target = VertexId::new();
    let edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            ori_domain::Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            },
            ori_domain::Vertex {
                id: end,
                position: Point2::new(1.0, 2.0),
            },
            ori_domain::Vertex {
                id: target,
                position: target_position,
            },
        ],
        edges: vec![ori_domain::Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Auxiliary,
        }],
    };
    GeometryReferenceFixture {
        project: ProjectState::new(pattern),
        edge,
        target,
        deterministic_endpoint: deterministic_edge_geometry(1.0, 2.0),
    }
}

fn polar_reference_fixture(target_position: Point2) -> PolarReferenceFixture {
    let start = VertexId::new();
    let target = VertexId::new();
    let start_position = Point2::new(1.25, -2.5);
    let length_mm = 3.75;
    let angle_degrees = 37.5;
    let (x, y) = ori_numeric::deterministic_polar_endpoint_v2(
        start_position.x,
        start_position.y,
        length_mm,
        angle_degrees,
    )
    .expect("deterministic polar endpoint");
    PolarReferenceFixture {
        project: ProjectState::new(CreasePattern {
            vertices: vec![
                ori_domain::Vertex {
                    id: start,
                    position: start_position,
                },
                ori_domain::Vertex {
                    id: target,
                    position: target_position,
                },
            ],
            edges: Vec::new(),
        }),
        start,
        target,
        start_position,
        length_mm,
        angle_degrees,
        deterministic_endpoint: Point2::new(x, y),
    }
}

fn polar_binding(
    fixture: &PolarReferenceFixture,
    endpoint: Point2,
    deterministic_v2: bool,
) -> VertexCoordinateExpressions {
    let mut binding = VertexCoordinateExpressions::new(
        fixture.target,
        endpoint.x.to_string(),
        endpoint.y.to_string(),
        endpoint.x,
        endpoint.y,
    );
    if deterministic_v2 {
        binding.schema_version =
            ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2;
        binding.transcendental_model_id =
            Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.to_owned());
    }
    binding.polar_construction = Some(PolarVertexConstructionExpressions {
        schema_version: ori_formats::PROJECT_NUMERIC_EXPRESSIONS_SCHEMA_VERSION,
        start_vertex: fixture.start,
        adopted_start_x_mm: fixture.start_position.x,
        adopted_start_y_mm: fixture.start_position.y,
        length_source: fixture.length_mm.to_string(),
        angle_degrees_source: fixture.angle_degrees.to_string(),
        adopted_length_mm: fixture.length_mm,
        adopted_angle_degrees: fixture.angle_degrees,
    });
    binding
}

fn deterministic_binding(fixture: &GeometryReferenceFixture) -> VertexCoordinateExpressions {
    let mut binding = VertexCoordinateExpressions::new(
        fixture.target,
        edge_reference(fixture.edge, "length"),
        edge_reference(fixture.edge, "angle"),
        fixture.deterministic_endpoint.x,
        fixture.deterministic_endpoint.y,
    );
    upgrade_expression_binding_after_deterministic_reevaluation(&mut binding, true);
    binding
}

fn first_history_binding_mut(archive: &mut Ori2ProjectArchive) -> &mut VertexCoordinateExpressions {
    archive
        .document
        .numeric_expressions
        .vertex_undo_stack
        .iter_mut()
        .chain(
            archive
                .document
                .numeric_expressions
                .vertex_redo_stack
                .iter_mut(),
        )
        .flatten()
        .flat_map(|transition| transition.changes.iter_mut())
        .flat_map(|change| change.before.iter_mut().chain(change.after.iter_mut()))
        .next()
        .expect("geometry-reference history binding")
}

#[test]
fn legacy_geometry_reference_adopts_creator_bits_without_replaying_the_creator_libm() {
    let deterministic_endpoint = deterministic_edge_geometry(1.0, 2.0);
    let creator_endpoint = Point2::new(
        f64::from_bits(deterministic_endpoint.x.to_bits() + 1),
        f64::from_bits(deterministic_endpoint.y.to_bits() + 1),
    );
    let mut fixture = geometry_reference_fixture(creator_endpoint);
    let mut binding = deterministic_binding(&fixture);
    binding.schema_version = ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_LEGACY_V1;
    binding.transcendental_model_id = None;
    binding.adopted_x_mm = creator_endpoint.x;
    binding.adopted_y_mm = creator_endpoint.y;
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];

    let document = fixture.project.document();
    validate_loaded_numeric_expression_bindings(&document)
        .expect("legacy creator-runtime endpoint bits remain loadable");
    let bytes = ori_formats::write_project_ori2(&document).expect("write legacy project");
    let restored = ori_formats::read_project_ori2(&bytes).expect("read legacy project");
    validate_loaded_numeric_expression_bindings(&restored)
        .expect("legacy project round trip remains loadable");
    let staged = ProjectState::from_valid_document(restored, PathBuf::new());
    assert_eq!(
        reevaluate_saved_vertex_expressions_for_archive_load(&staged)
            .expect("legacy load evaluation"),
        vec![(fixture.target, creator_endpoint)]
    );
    assert_eq!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&staged, true)
            .expect("explicit supported-model reevaluation"),
        vec![(fixture.target, deterministic_endpoint)]
    );
    if ori_numeric::deterministic_transcendental_model_supported_v1() {
        assert_eq!(
            reevaluate_saved_vertex_expressions(&staged)
                .expect("explicit runtime reevaluation on supported target"),
            vec![(fixture.target, deterministic_endpoint)]
        );
    } else {
        assert_eq!(
            reevaluate_saved_vertex_expressions(&staged),
            Err("deterministic geometry reference model is unsupported".to_owned())
        );
    }

    let mut upgraded = staged.numeric_expressions.vertex_coordinates[0].clone();
    upgraded.adopted_x_mm = deterministic_endpoint.x;
    upgraded.adopted_y_mm = deterministic_endpoint.y;
    upgrade_expression_binding_after_deterministic_reevaluation(&mut upgraded, true);
    assert_eq!(
        upgraded.schema_version,
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2
    );
    assert_eq!(
        upgraded.transcendental_model_id.as_deref(),
        Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1)
    );
    let mut upgraded_document = staged.document();
    upgraded_document.numeric_expressions.vertex_coordinates = vec![upgraded];
    upgraded_document
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == fixture.target)
        .expect("upgraded target")
        .position = deterministic_endpoint;
    upgraded_document.thumbnail_svg = None;
    let upgraded_bytes =
        ori_formats::write_project_ori2(&upgraded_document).expect("write upgraded project");
    assert!(
        ori2_required_features(&upgraded_bytes)
            .contains(&ori_formats::ORI2_FEATURE_DETERMINISTIC_GEOMETRY_REFERENCES_V2.to_owned())
    );

    let mut dangling = document;
    dangling.numeric_expressions.vertex_coordinates[0].x_source =
        edge_reference(EdgeId::new(), "length");
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&dangling),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );
}

#[test]
fn legacy_geometry_reference_adopts_only_the_axis_with_a_strict_edge_token() {
    let creator_edge_value = 17.25;
    let stale_non_edge_value = -91.0;
    let referenced_vertex = vertex_id_ending_in_e();
    let mut fixture =
        geometry_reference_fixture(Point2::new(creator_edge_value, stale_non_edge_value));
    let mut pattern = fixture.project.editor.pattern().clone();
    pattern.vertices.push(ori_domain::Vertex {
        id: referenced_vertex,
        position: Point2::new(7.5, 8.5),
    });
    fixture.project = ProjectState::new(pattern);
    let referenced_vertex_wire = serde_json::to_value(referenced_vertex)
        .expect("vertex ID")
        .as_str()
        .expect("wire vertex ID")
        .to_owned();
    let x_source = edge_reference(fixture.edge, "length");
    let y_source = format!("v.{referenced_vertex_wire}.x");
    let mut binding = VertexCoordinateExpressions::new(
        fixture.target,
        x_source.clone(),
        y_source.clone(),
        creator_edge_value,
        stale_non_edge_value,
    );
    binding.schema_version = ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_LEGACY_V1;
    binding.transcendental_model_id = None;
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];

    assert!(ori_formats::source_uses_edge_geometry_reference(&x_source));
    assert!(!ori_formats::source_uses_edge_geometry_reference(&y_source));
    assert!(
        fixture.project.numeric_expressions.vertex_coordinates[0]
            .uses_legacy_edge_geometry_reference_v1()
    );
    assert_eq!(
        reevaluate_saved_vertex_expressions_for_archive_load(&fixture.project)
            .expect("legacy load evaluation"),
        vec![(fixture.target, Point2::new(creator_edge_value, 7.5))]
    );
}

#[test]
fn pure_vertex_reference_ending_in_e_does_not_require_the_edge_geometry_model() {
    let referenced_vertex = vertex_id_ending_in_e();
    let target = VertexId::new();
    let referenced_position = Point2::new(7.5, 8.5);
    let referenced_vertex_wire = serde_json::to_value(referenced_vertex)
        .expect("vertex ID")
        .as_str()
        .expect("wire vertex ID")
        .to_owned();
    let x_source = format!("v.{referenced_vertex_wire}.x");
    let y_source = format!("v.{referenced_vertex_wire}.y");
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![
            ori_domain::Vertex {
                id: referenced_vertex,
                position: referenced_position,
            },
            ori_domain::Vertex {
                id: target,
                position: Point2::new(0.0, 0.0),
            },
        ],
        edges: Vec::new(),
    });
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        target,
        x_source.clone(),
        y_source.clone(),
        0.0,
        0.0,
    )];

    assert!(!ori_formats::source_uses_edge_geometry_reference(&x_source));
    assert!(!ori_formats::source_uses_edge_geometry_reference(&y_source));
    assert_eq!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&project, false)
            .expect("pure vertex references do not use deterministic edge geometry"),
        vec![(target, referenced_position)]
    );
}

#[test]
fn saved_reference_tokens_require_a_left_lexical_boundary_before_expansion() {
    let mut fixture = geometry_reference_fixture(Point2::new(0.0, 0.0));
    let embedded_edge_source = format!("sqrt{}", edge_reference(fixture.edge, "length"));
    assert!(embedded_edge_source.starts_with("sqrte."));
    assert!(!ori_formats::source_uses_edge_geometry_reference(
        &embedded_edge_source
    ));
    fixture.project.numeric_expressions.vertex_coordinates =
        vec![VertexCoordinateExpressions::new(
            fixture.target,
            embedded_edge_source,
            "0",
            0.0,
            0.0,
        )];
    assert!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&fixture.project, true)
            .is_err(),
        "an embedded edge marker must not become a valid sqrt expression after expansion"
    );

    let referenced_vertex = fixture.project.editor.pattern().vertices[0].id;
    let referenced_vertex_wire = serde_json::to_value(referenced_vertex)
        .expect("vertex ID")
        .as_str()
        .expect("wire vertex ID")
        .to_owned();
    let embedded_vertex_source = format!("sqrtv.{referenced_vertex_wire}.x");
    fixture.project.numeric_expressions.vertex_coordinates =
        vec![VertexCoordinateExpressions::new(
            fixture.target,
            embedded_vertex_source,
            "0",
            0.0,
            0.0,
        )];
    assert!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&fixture.project, true)
            .is_err(),
        "an embedded vertex marker must not become a valid sqrt expression after expansion"
    );
}

#[test]
fn deterministic_geometry_reference_is_bit_exact_and_unknown_metadata_fails_closed() {
    let deterministic_endpoint = deterministic_edge_geometry(1.0, 2.0);
    let mut fixture = geometry_reference_fixture(deterministic_endpoint);
    let binding = deterministic_binding(&fixture);
    assert_eq!(
        binding.schema_version,
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2
    );
    assert_eq!(
        binding.transcendental_model_id.as_deref(),
        Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1)
    );
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];
    let document = fixture.project.document();
    validate_loaded_numeric_expression_bindings_with_model_support(&document, true)
        .expect("deterministic geometry endpoint is bit-exact");
    if !ori_numeric::deterministic_transcendental_model_supported_v1() {
        assert_eq!(
            validate_loaded_numeric_expression_bindings(&document),
            Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
        );
    }

    let mut one_ulp = document.clone();
    let forged_x = f64::from_bits(
        one_ulp.numeric_expressions.vertex_coordinates[0]
            .adopted_x_mm
            .to_bits()
            + 1,
    );
    one_ulp.numeric_expressions.vertex_coordinates[0].adopted_x_mm = forged_x;
    one_ulp
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == fixture.target)
        .expect("target")
        .position
        .x = forged_x;
    assert_eq!(
        validate_loaded_numeric_expression_bindings_with_model_support(&one_ulp, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    for (schema_version, model) in [
        (
            ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_LEGACY_V1,
            Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1),
        ),
        (
            ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2,
            None,
        ),
        (
            ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2,
            Some("forged_model"),
        ),
        (u32::MAX, None),
    ] {
        let mut forged = document.clone();
        let binding = &mut forged.numeric_expressions.vertex_coordinates[0];
        binding.schema_version = schema_version;
        binding.transcendental_model_id = model.map(str::to_owned);
        assert_eq!(
            validate_loaded_numeric_expression_bindings_with_model_support(&forged, true),
            Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
        );
    }
}

#[test]
fn deterministic_geometry_reference_survives_history_and_project_round_trip() {
    let deterministic_endpoint = deterministic_edge_geometry(1.0, 2.0);
    let mut fixture = geometry_reference_fixture(Point2::new(-1.0, -1.0));
    let instance_id = fixture.project.instance_id;
    let project_id = fixture.project.project_id;
    execute_expected_command(
        &mut fixture.project,
        ProjectExpectation::new(instance_id, project_id, 0),
        Command::MoveVertex {
            id: fixture.target,
            position: deterministic_endpoint,
        },
    )
    .expect("move expression target");
    fixture.deterministic_endpoint = deterministic_endpoint;
    let binding = deterministic_binding(&fixture);
    assert_eq!(
        binding.schema_version,
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2
    );
    fixture.project.adopt_vertex_coordinate_expression(binding);
    assert_eq!(
        fixture.project.numeric_expressions.vertex_coordinates[0].schema_version,
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2
    );

    let archive = fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("save-time deterministic reauthentication");
    assert_eq!(
        archive.document.numeric_expressions.vertex_coordinates[0].schema_version,
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2
    );
    validate_loaded_numeric_expression_archive_with_model_support(&archive, true)
        .expect("load-time current and history reauthentication");

    let mut one_ulp_history = archive.clone();
    let history_binding = first_history_binding_mut(&mut one_ulp_history);
    history_binding.adopted_x_mm = f64::from_bits(history_binding.adopted_x_mm.to_bits() + 1);
    assert_eq!(
        validate_loaded_numeric_expression_archive_with_model_support(&one_ulp_history, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut malformed_history = archive.clone();
    let history_binding = first_history_binding_mut(&mut malformed_history);
    history_binding.x_source.push_str("junk");
    assert_eq!(
        validate_loaded_numeric_expression_archive_with_model_support(&malformed_history, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut forged_model_history = archive.clone();
    first_history_binding_mut(&mut forged_model_history).transcendental_model_id =
        Some("forged_model".to_owned());
    assert_eq!(
        validate_loaded_numeric_expression_archive_with_model_support(&forged_model_history, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    if !ori_numeric::deterministic_transcendental_model_supported_v1() {
        assert_eq!(
            fixture.project.project_archive(),
            Err(PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())
        );
        assert_eq!(
            validate_loaded_numeric_expression_archive(&archive),
            Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
        );
    }
    let bytes = ori_formats::write_project_archive_ori2(&archive).expect("write project archive");
    let restored_archive =
        ori_formats::read_project_archive_ori2(&bytes).expect("read project archive");
    let mut reopened = ProjectState::from_project_archive(
        restored_archive,
        PathBuf::from("geometry-reference-round-trip.ori2"),
    )
    .expect("restore project archive");

    let binding = &reopened.numeric_expressions.vertex_coordinates[0];
    assert_eq!(
        binding.schema_version,
        ori_formats::VERTEX_COORDINATE_EXPRESSIONS_SCHEMA_VERSION_DETERMINISTIC_V2
    );
    assert_eq!(
        binding.transcendental_model_id.as_deref(),
        Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1)
    );
    let reopened_instance_id = reopened.instance_id;
    let reopened_project_id = reopened.project_id;
    let revision = reopened.editor.revision();
    execute_undo(
        &mut reopened,
        reopened_instance_id,
        reopened_project_id,
        revision,
    )
    .expect("undo deterministic expression edit");
    assert!(reopened.numeric_expressions.vertex_coordinates.is_empty());
    let revision = reopened.editor.revision();
    execute_redo(
        &mut reopened,
        reopened_instance_id,
        reopened_project_id,
        revision,
    )
    .expect("redo deterministic expression edit");
    let restored = &reopened.numeric_expressions.vertex_coordinates[0];
    assert_eq!(
        restored.transcendental_model_id.as_deref(),
        Some(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1)
    );
    assert_eq!(
        reevaluate_saved_vertex_expressions_with_model_support_for_test(&reopened, true)
            .expect("replay restored expression"),
        vec![(fixture.target, deterministic_endpoint)]
    );

    let mut forged_document = fixture.project.document();
    let forged_x = f64::from_bits(
        forged_document.numeric_expressions.vertex_coordinates[0]
            .adopted_x_mm
            .to_bits()
            + 1,
    );
    forged_document.numeric_expressions.vertex_coordinates[0].adopted_x_mm = forged_x;
    forged_document
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == fixture.target)
        .expect("target")
        .position
        .x = forged_x;
    let forged = ProjectState::from_valid_document(forged_document, PathBuf::from("forged.ori2"));
    assert_eq!(
        forged.project_archive_with_geometry_reference_model_support(true),
        Err(PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())
    );
}

#[test]
fn polar_current_save_rejects_one_ulp_start_drift() {
    let endpoint = polar_reference_fixture(Point2::new(0.0, 0.0)).deterministic_endpoint;
    let mut fixture = polar_reference_fixture(endpoint);
    let binding = polar_binding(&fixture, endpoint, false);
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];
    let mut forged_document = fixture.project.document();
    forged_document
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == fixture.start)
        .expect("polar start")
        .position
        .x = f64::from_bits(fixture.start_position.x.to_bits() + 1);
    let forged =
        ProjectState::from_valid_document(forged_document, PathBuf::from("forged-start.ori2"));

    assert_eq!(
        forged.project_archive_with_geometry_reference_model_support(true),
        Err(PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())
    );
}

#[test]
fn polar_start_move_and_removal_invalidate_the_binding_and_undo_restores_it() {
    let endpoint = polar_reference_fixture(Point2::new(0.0, 0.0)).deterministic_endpoint;
    let mut fixture = polar_reference_fixture(endpoint);
    let binding = polar_binding(&fixture, endpoint, false);
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];
    let instance_id = fixture.project.instance_id;
    let project_id = fixture.project.project_id;

    execute_expected_command(
        &mut fixture.project,
        ProjectExpectation::new(instance_id, project_id, 0),
        Command::MoveVertex {
            id: fixture.start,
            position: Point2::new(
                f64::from_bits(fixture.start_position.x.to_bits() + 1),
                fixture.start_position.y,
            ),
        },
    )
    .expect("move polar start");
    assert!(
        fixture
            .project
            .numeric_expressions
            .vertex_coordinates
            .is_empty()
    );
    fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("moved polar start no longer blocks save");

    let revision = fixture.project.editor.revision();
    execute_undo(&mut fixture.project, instance_id, project_id, revision)
        .expect("undo polar start move");
    assert_eq!(
        fixture.project.numeric_expressions.vertex_coordinates.len(),
        1
    );
    fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("undo restores a valid polar binding");

    let revision = fixture.project.editor.revision();
    execute_expected_command(
        &mut fixture.project,
        ProjectExpectation::new(instance_id, project_id, revision),
        Command::RemoveVertex { id: fixture.start },
    )
    .expect("remove polar start");
    assert!(
        fixture
            .project
            .numeric_expressions
            .vertex_coordinates
            .is_empty()
    );
    fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("removed polar start no longer blocks save");

    let revision = fixture.project.editor.revision();
    execute_undo(&mut fixture.project, instance_id, project_id, revision)
        .expect("undo polar start removal");
    assert_eq!(
        fixture.project.numeric_expressions.vertex_coordinates.len(),
        1
    );
    fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("undo removal restores a valid polar binding");
}

#[test]
fn constraint_solve_does_not_readopt_a_polar_binding_after_moving_its_start() {
    let endpoint = polar_reference_fixture(Point2::new(0.0, 0.0)).deterministic_endpoint;
    let mut fixture = polar_reference_fixture(endpoint);
    let binding = polar_binding(&fixture, endpoint, false);
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding.clone()];
    let instance_id = fixture.project.instance_id;
    let project_id = fixture.project.project_id;
    let token = ProjectId::new();
    let stage = GeometricConstraintSolveStage {
        token,
        project_instance_id: instance_id,
        project_id,
        revision: 0,
        positions: vec![(
            fixture.start,
            Point2::new(
                f64::from_bits(fixture.start_position.x.to_bits() + 1),
                fixture.start_position.y,
            ),
        )],
        expression_bindings: Some(vec![binding]),
        exact_satisfaction: None,
    };

    apply_geometric_constraint_solve_stage(
        &mut fixture.project,
        &stage,
        instance_id,
        project_id,
        0,
        token,
    )
    .expect("apply constraint stage that moves a polar start");
    assert!(
        fixture
            .project
            .numeric_expressions
            .vertex_coordinates
            .is_empty()
    );
    fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("constraint solve does not leave an unsavable polar binding");

    let revision = fixture.project.editor.revision();
    execute_undo(&mut fixture.project, instance_id, project_id, revision)
        .expect("undo constraint solve");
    assert_eq!(
        fixture.project.numeric_expressions.vertex_coordinates.len(),
        1
    );
    fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("undo restores the polar binding and start together");
}

#[test]
fn history_only_polar_load_rejects_one_ulp_start_drift() {
    let mut fixture = polar_reference_fixture(Point2::new(-1.0, -1.0));
    let instance_id = fixture.project.instance_id;
    let project_id = fixture.project.project_id;
    execute_expected_command(
        &mut fixture.project,
        ProjectExpectation::new(instance_id, project_id, 0),
        Command::MoveVertex {
            id: fixture.target,
            position: fixture.deterministic_endpoint,
        },
    )
    .expect("move polar target");
    let binding = polar_binding(&fixture, fixture.deterministic_endpoint, false);
    fixture.project.adopt_vertex_coordinate_expression(binding);
    execute_undo(&mut fixture.project, instance_id, project_id, 1)
        .expect("move polar binding into redo-only history");
    assert!(
        fixture
            .project
            .numeric_expressions
            .vertex_coordinates
            .is_empty()
    );

    let archive = fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("valid history-only legacy polar archive");
    let mut one_ulp_history = archive.clone();
    let polar = first_history_binding_mut(&mut one_ulp_history)
        .polar_construction
        .as_mut()
        .expect("history-only polar metadata");
    polar.adopted_start_x_mm = f64::from_bits(polar.adopted_start_x_mm.to_bits() + 1);
    assert_eq!(
        validate_loaded_numeric_expression_archive_with_model_support(&one_ulp_history, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );
}

#[test]
fn history_only_polar_v2_load_rejects_coordinate_source_only_tampering() {
    let mut fixture = polar_reference_fixture(Point2::new(-1.0, -1.0));
    let instance_id = fixture.project.instance_id;
    let project_id = fixture.project.project_id;
    execute_expected_command(
        &mut fixture.project,
        ProjectExpectation::new(instance_id, project_id, 0),
        Command::MoveVertex {
            id: fixture.target,
            position: fixture.deterministic_endpoint,
        },
    )
    .expect("move polar target");
    let binding = polar_binding(&fixture, fixture.deterministic_endpoint, true);
    fixture.project.adopt_vertex_coordinate_expression(binding);
    execute_undo(&mut fixture.project, instance_id, project_id, 1)
        .expect("move deterministic polar binding into redo-only history");
    assert!(
        fixture
            .project
            .numeric_expressions
            .vertex_coordinates
            .is_empty()
    );

    let archive = fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("valid history-only deterministic polar archive");
    let mut forged = archive.clone();
    first_history_binding_mut(&mut forged).x_source = "0".to_owned();
    assert_eq!(
        validate_loaded_numeric_expression_archive_with_model_support(&forged, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );
}

#[test]
fn legacy_polar_v1_adopts_creator_endpoint_bits_without_native_trig_replay() {
    let deterministic = polar_reference_fixture(Point2::new(0.0, 0.0)).deterministic_endpoint;
    let creator_endpoint = Point2::new(
        f64::from_bits(deterministic.x.to_bits() + 1),
        f64::from_bits(deterministic.y.to_bits() + 1),
    );
    let mut fixture = polar_reference_fixture(creator_endpoint);
    let binding = polar_binding(&fixture, creator_endpoint, false);
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];
    let document = fixture.project.document();

    validate_loaded_numeric_expression_bindings_with_model_support(&document, true)
        .expect("legacy creator endpoint loads on a supported target");
    validate_loaded_numeric_expression_bindings_with_model_support(&document, false)
        .expect("legacy creator endpoint loads without deterministic model support");
    let archive = fixture
        .project
        .project_archive_with_geometry_reference_model_support(false)
        .expect("legacy creator endpoint saves without deterministic model support");
    let bytes = ori_formats::write_project_archive_ori2(&archive).expect("write legacy polar");
    let restored = ori_formats::read_project_archive_ori2(&bytes).expect("read legacy polar");
    validate_loaded_numeric_expression_archive_with_model_support(&restored, false)
        .expect("legacy polar round trip preserves saved endpoint authority");
    let binding = &restored.document.numeric_expressions.vertex_coordinates[0];
    assert_eq!(binding.adopted_x_mm.to_bits(), creator_endpoint.x.to_bits());
    assert_eq!(binding.adopted_y_mm.to_bits(), creator_endpoint.y.to_bits());
}

#[test]
fn polar_v2_current_load_rejects_coordinate_source_only_tampering() {
    let endpoint = polar_reference_fixture(Point2::new(0.0, 0.0)).deterministic_endpoint;
    let mut fixture = polar_reference_fixture(endpoint);
    let binding = polar_binding(&fixture, endpoint, true);
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];
    let mut forged = fixture.project.document();
    forged.numeric_expressions.vertex_coordinates[0].x_source = "0".to_owned();

    assert_eq!(
        validate_loaded_numeric_expression_bindings_with_model_support(&forged, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );
}

#[test]
fn polar_v2_generation_and_acceptance_fail_closed_without_model_support() {
    let endpoint = polar_reference_fixture(Point2::new(0.0, 0.0)).deterministic_endpoint;
    let mut fixture = polar_reference_fixture(endpoint);
    let binding = polar_binding(&fixture, endpoint, true);
    fixture.project.numeric_expressions.vertex_coordinates = vec![binding];
    let document = fixture.project.document();

    validate_loaded_numeric_expression_bindings_with_model_support(&document, true)
        .expect("supported target accepts deterministic polar V2");
    assert_eq!(
        validate_loaded_numeric_expression_bindings_with_model_support(&document, false),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );
    let mut one_ulp_endpoint = document.clone();
    let forged_x = f64::from_bits(
        one_ulp_endpoint.numeric_expressions.vertex_coordinates[0]
            .adopted_x_mm
            .to_bits()
            + 1,
    );
    let forged_binding = &mut one_ulp_endpoint.numeric_expressions.vertex_coordinates[0];
    forged_binding.x_source = forged_x.to_string();
    forged_binding.adopted_x_mm = forged_x;
    one_ulp_endpoint
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == fixture.target)
        .expect("polar target")
        .position
        .x = forged_x;
    assert_eq!(
        validate_loaded_numeric_expression_bindings_with_model_support(&one_ulp_endpoint, true),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );
    let forged_project =
        ProjectState::from_valid_document(one_ulp_endpoint, PathBuf::from("forged-polar.ori2"));
    assert_eq!(
        forged_project.project_archive_with_geometry_reference_model_support(true),
        Err(PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())
    );
    assert_eq!(
        fixture
            .project
            .project_archive_with_geometry_reference_model_support(false),
        Err(PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())
    );
    assert_eq!(
        pattern_edit_commands::deterministic_polar_endpoint_with_model_support(
            fixture.start_position,
            fixture.length_mm,
            fixture.angle_degrees,
            true,
        )
        .expect("supported generation"),
        endpoint
    );
    assert_eq!(
        pattern_edit_commands::deterministic_polar_endpoint_with_model_support(
            fixture.start_position,
            fixture.length_mm,
            fixture.angle_degrees,
            false,
        ),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let archive = fixture
        .project
        .project_archive_with_geometry_reference_model_support(true)
        .expect("supported deterministic polar archive");
    let bytes = ori_formats::write_project_archive_ori2(&archive).expect("write polar V2");
    assert!(
        ori2_required_features(&bytes)
            .contains(&ori_formats::ORI2_FEATURE_DETERMINISTIC_GEOMETRY_REFERENCES_V2.to_owned())
    );
}
