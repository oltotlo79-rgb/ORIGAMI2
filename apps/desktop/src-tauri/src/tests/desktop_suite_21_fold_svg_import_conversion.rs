#[test]
fn fold_import_applies_valley_cut_and_ignore_mapping_with_scale() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "unit",
        "vertices_coords": [
            [0.0, 0.0], [2.0, 0.0], [4.0, 0.0],
            [4.0, 4.0], [2.0, 4.0], [0.0, 4.0]
        ],
        "edges_vertices": [
            [0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 0],
            [0, 3], [0, 4], [1, 3], [2, 5]
        ],
        "edges_assignment": ["B", "B", "B", "B", "B", "B", "M", "V", "C", "F"]
    }))
    .expect("serialize mapped FOLD fixture");
    let replacement = build_fold_import_replacement(
        &bytes,
        "複数線種".to_owned(),
        2.5,
        FoldBoundaryCandidateId(0),
        HashMap::from([
            ("M".to_owned(), FoldImportTargetRequest::Mountain),
            ("V".to_owned(), FoldImportTargetRequest::Valley),
            ("C".to_owned(), FoldImportTargetRequest::Cut),
            ("F".to_owned(), FoldImportTargetRequest::Ignore),
        ]),
    )
    .expect("convert explicit mapped assignments");
    let edges = &replacement.editor.pattern().edges;

    assert_eq!(edges.len(), 9);
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
            .count(),
        6
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        1
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Valley)
            .count(),
        1
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Cut)
            .count(),
        1
    );
    assert!(replacement.editor.paper().cutting_allowed);
    assert!(
        replacement
            .editor
            .pattern()
            .vertices
            .iter()
            .any(|vertex| vertex.position == Point2::new(10.0, 10.0))
    );
}

#[test]
fn fold_import_preview_truncation_remaps_every_rendered_endpoint() {
    let interior_edge_count = MAX_FOLD_IMPORT_PREVIEW_EDGES - 3;
    let mut vertices = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut edges = Vec::new();
    let mut assignments = Vec::new();
    for index in 0..interior_edge_count {
        let x = 10.0 + index as f64;
        let start = vertices.len();
        vertices.push([x, 2.0]);
        vertices.push([x, 3.0]);
        edges.push([start, start + 1]);
        assignments.push("F");
    }
    edges.extend([[0_usize, 1_usize], [1, 2], [2, 3], [3, 0]]);
    assignments.extend(["B"; 4]);
    let bytes = serde_json::to_vec(&serde_json::json!({
        "vertices_coords": vertices,
        "edges_vertices": edges,
        "edges_assignment": assignments,
        "file_classes": ["singleModel"]
    }))
    .expect("serialize large preview fixture");
    let preview = read_fold_preview(&bytes).expect("read large preview");
    let response = fold_import_preview_snapshot(ProjectId::new(), &preview);

    assert!(response.preview_truncated);
    assert_eq!(response.preview_edges.len(), MAX_FOLD_IMPORT_PREVIEW_EDGES);
    assert!(response.preview_vertices.len() < response.vertex_count);
    assert!(response.preview_edges.iter().all(|edge| {
        edge.start < response.preview_vertices.len() && edge.end < response.preview_vertices.len()
    }));
    assert_eq!(
        response
            .preview_edges
            .iter()
            .filter(|edge| edge.assignment == "B")
            .count(),
        4
    );
    assert_eq!(
        response
            .assignments
            .iter()
            .map(|summary| summary.assignment.as_str())
            .collect::<Vec<_>>(),
        vec!["B", "F"]
    );
    assert!(response.warnings.iter().all(|warning| !warning.is_ascii()));
    assert!(
        response
            .warnings
            .iter()
            .any(|warning| warning.contains("ファイル分類"))
    );
}

#[test]
fn svg_import_file_errors_do_not_expose_the_selected_path() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-design.svg";
    let path = directory.join(secret_name);

    let missing_error =
        read_svg_import_bytes(&path).expect_err("missing SVG import must be rejected");
    assert_eq!(missing_error, SVG_FILE_OPEN_FAILED_MESSAGE);
    assert!(!missing_error.contains(secret_name));
    assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!missing_error.to_ascii_lowercase().contains("os error"));

    fs::write(
        &path,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><SECRET_MARKER></OTHER_SECRET></svg>"#,
    )
    .expect("write malformed SVG fixture");
    let malformed_error =
        load_svg_import_preview(&path).expect_err("malformed SVG import must be rejected");
    assert_eq!(malformed_error, SVG_FILE_INVALID_MESSAGE);
    assert!(!malformed_error.contains("SECRET_MARKER"));
    assert!(!malformed_error.contains("OTHER_SECRET"));
    assert!(!malformed_error.contains(secret_name));

    File::create(&path)
        .expect("create oversized SVG fixture")
        .set_len(MAX_SVG_IMPORT_FILE_SIZE + 1)
        .expect("make sparse oversized SVG fixture");
    let oversized_error =
        read_svg_import_bytes(&path).expect_err("oversized SVG import must be rejected");
    assert_eq!(oversized_error, SVG_FILE_TOO_LARGE_MESSAGE);
    assert!(!oversized_error.contains(secret_name));
    assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!oversized_error.contains(&(MAX_SVG_IMPORT_FILE_SIZE + 1).to_string()));
}

#[test]
fn svg_import_warning_messages_do_not_echo_source_style_values() {
    for kind in [
        SvgWarningKind::UnsupportedCssSelector("#SECRET_SELECTOR".to_owned()),
        SvgWarningKind::UnsupportedPaint("url(SECRET_PAINT)".to_owned()),
        SvgWarningKind::UnsupportedLengthUnit("SECRET_LENGTH".to_owned()),
    ] {
        let message = svg_import_warning_message(&SvgPreviewWarning {
            kind,
            occurrences: 1,
        });
        assert!(!message.contains("SECRET"));
    }

    let source = br##"<svg xmlns="http://www.w3.org/2000/svg"
                              viewBox="0 0 10 10" width="10mm" height="10mm"
                              fill="none">
              <line stroke="#111111" stroke-linecap="SECRET_LINE_CAP"
                    x1="0" y1="0" x2="10" y2="10"/>
            </svg>"##;
    let preview = read_svg_preview(source).expect("parse unknown line-cap fixture");
    assert_eq!(
        preview.warnings(),
        &[SvgPreviewWarning {
            kind: SvgWarningKind::UnsupportedAttribute("stroke-linecap".to_owned()),
            occurrences: 1,
        }]
    );
    let response = svg_import_preview_snapshot(ProjectId::new(), &preview)
        .expect("build unknown line-cap snapshot");
    let encoded = serde_json::to_string(&response).expect("serialize SVG preview snapshot");
    assert!(!encoded.contains("SECRET"));
    assert!(!encoded.contains("LINE_CAP"));
}

#[test]
fn svg_import_preview_contract_and_conversion_create_a_valid_editable_project() {
    let source = r##"<?xml version="1.0" encoding="UTF-8"?>
            <svg xmlns="http://www.w3.org/2000/svg"
                 viewBox="0 0 100 100" width="100mm" height="100mm">
              <title>  SVG取込テスト  </title>
              <rect x="0" y="0" width="100" height="100"
                    fill="none" stroke="#222222" data-origami-kind="boundary"/>
              <line id="main-fold" x1="0" y1="0" x2="100" y2="100"
                    stroke="#cc3344" stroke-linecap="round"
                    data-origami-kind="mountain"/>
            </svg>"##;
    let bytes = source.as_bytes();
    let preview = read_svg_preview(bytes).expect("read SVG preview");
    let import_id = ProjectId::new();
    let response =
        svg_import_preview_snapshot(import_id, &preview).expect("build bounded SVG preview");

    assert_eq!(response.import_id, import_id);
    assert_eq!(response.file_name, SVG_IMPORT_FILE_LABEL);
    assert_eq!(response.suggested_name, "SVG取込テスト");
    assert_eq!(response.default_mm_per_unit, Some(1.0));
    assert_eq!(
        response.root_view_box,
        Some(SvgRootViewBox {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        })
    );
    assert_eq!(response.root_physical_size.width_millimetres, Some(100.0));
    assert_eq!(response.root_physical_size.height_millimetres, Some(100.0));
    assert_eq!(response.source_segment_count, 5);
    assert_eq!(response.style_groups.len(), 2);
    assert!(response.style_groups.iter().all(|group| {
        group.element_count > 0
            && group.segment_count > 0
            && matches!(
                group.line_cap,
                SvgLineCap::Butt | SvgLineCap::Round | SvgLineCap::Square
            )
            && group
                .stroke_color
                .as_deref()
                .is_some_and(|color| color.starts_with('#'))
    }));
    let main_fold_group = response
        .style_groups
        .iter()
        .find(|group| group.representative_id.as_deref() == Some("main-fold"))
        .expect("main fold style group");
    assert_eq!(main_fold_group.element_count, 1);
    assert_eq!(main_fold_group.segment_count, 1);
    assert_eq!(main_fold_group.line_cap, SvgLineCap::Round);
    assert_eq!(
        serde_json::to_value(main_fold_group)
            .expect("serialize SVG style group snapshot")
            .get("line_cap")
            .and_then(serde_json::Value::as_str),
        Some("round")
    );
    assert_eq!(response.preview_edges.len(), 5);
    assert!(!response.preview_truncated);
    assert!(response.preview_edges.iter().all(|edge| {
        edge.start < response.preview_vertices.len() && edge.end < response.preview_vertices.len()
    }));
    assert!(
        response
            .boundary_candidates
            .iter()
            .any(|candidate| candidate.kind == "view_box")
    );
    assert!(
        response
            .boundary_candidates
            .iter()
            .any(|candidate| candidate.kind == "rectangle")
    );
    assert!(response.boundary_candidates.iter().all(|candidate| {
        candidate.segment_count == candidate.vertices.len() && candidate.segment_count >= 3
    }));
    assert!(
        response
            .warnings
            .iter()
            .any(|warning| warning.contains("data-origami-kind"))
    );

    let rectangle = preview
        .boundary_candidates()
        .iter()
        .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Rectangle)
        .expect("rectangle boundary candidate");
    let mappings: Vec<SvgGroupMapping> = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: match group.semantic.as_deref() {
                Some("mountain") => SvgGroupTarget::Mountain,
                _ => SvgGroupTarget::Ignore,
            },
        })
        .collect();
    let boundary_error = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "SVG取込テスト".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings.clone(),
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: false,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: false,
        },
    )
    .err()
    .expect("boundary must require explicit confirmation");
    assert!(boundary_error.contains("boundary must be explicitly confirmed"));
    let warning_error = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "SVG取込テスト".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings.clone(),
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: false,
            cutting_allowed_confirmed: false,
        },
    )
    .err()
    .expect("warnings must require explicit confirmation");
    assert!(warning_error.contains("warnings must be explicitly acknowledged"));
    let replacement = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "SVG取込テスト".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings,
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: false,
        },
    )
    .expect("convert SVG into a project");

    assert_eq!(replacement.name, "SVG取込テスト");
    assert_eq!(replacement.editor.pattern().edges.len(), 5);
    assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        1
    );
    assert!(!replacement.editor.paper().cutting_allowed);
    assert!(replacement.editor.instruction_timeline().steps.is_empty());
    assert_eq!(replacement.editor.revision(), 0);
    assert!(!replacement.editor.can_undo());
    assert!(!replacement.editor.can_redo());
    assert!(replacement.current_path.is_none());
    assert!(replacement.saved_document.is_none());
    assert!(replacement.is_dirty());
}

#[test]
fn svg_import_preview_rejects_more_than_sixty_four_warning_categories() {
    let mut source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"
                     width="100mm" height="100mm" fill="none" stroke="#111">
                   <title>{}</title>"##,
        "a".repeat(MAX_PROJECT_NAME_CHARS + 1)
    );
    for index in 0..63 {
        let class = if index == 0 { r#" class="fold""# } else { "" };
        source.push_str(&format!(
            r#"<line{class} unsupported{index}="x" x1="0" y1="{index}" x2="1" y2="{index}"/>"#
        ));
    }
    source.push_str("</svg>");

    let preview = read_svg_preview(source.as_bytes()).expect("bounded warning fixture");
    assert_eq!(preview.warnings().len(), 63);
    let error = svg_import_preview_snapshot(ProjectId::new(), &preview)
        .expect_err("synthetic warning categories must not be truncated");
    assert!(error.contains("more than 64"));
}

#[test]
fn svg_cut_mapping_requires_explicit_permission_and_splits_crossings() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
              <rect x="0" y="0" width="100" height="100"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
              <line x1="0" y1="0" x2="100" y2="100"
                    stroke="#c33" data-origami-kind="mountain"/>
              <line x1="0" y1="50" x2="100" y2="50"
                    stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read cut SVG preview");
    let rectangle = preview
        .boundary_candidates()
        .iter()
        .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Rectangle)
        .expect("rectangle boundary candidate");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: match group.semantic.as_deref() {
                Some("mountain") => SvgGroupTarget::Mountain,
                Some("cut") => SvgGroupTarget::Cut,
                _ => SvgGroupTarget::Ignore,
            },
        })
        .collect::<Vec<_>>();

    let error = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "切断確認".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings.clone(),
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: false,
        },
    )
    .err()
    .expect("cutting must require explicit confirmation");
    assert!(error.contains("cutting must be explicitly allowed"));

    let replacement = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "切断確認".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings,
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: true,
        },
    )
    .expect("confirmed cut SVG must convert");
    let edges = &replacement.editor.pattern().edges;
    assert!(replacement.editor.paper().cutting_allowed);
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        2,
        "the mountain line must split at the X intersection"
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Cut)
            .count(),
        2,
        "the cut line must split at the X intersection"
    );
    assert!(
        replacement.editor.paper().boundary_vertices.len() > 4,
        "cut contacts must split the paper boundary at both T junctions"
    );
}
