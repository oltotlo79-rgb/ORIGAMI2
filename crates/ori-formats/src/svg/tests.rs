use ori_geometry::validate_crease_pattern;

use super::*;

fn document(root_attributes: &str, body: &str) -> String {
    format!(
        r##"<svg xmlns="{SVG_NAMESPACE}" {root_attributes} stroke="#000" fill="none">{body}</svg>"##
    )
}

fn standard_document(body: &str) -> String {
    document(
        r#"viewBox="0 0 100 100" width="100mm" height="100mm""#,
        body,
    )
}

fn preview(body: &str) -> SvgPreview {
    let source = standard_document(body);
    read_svg_preview(source.as_bytes()).expect("SVG preview")
}

fn mappings(preview: &SvgPreview, target: SvgGroupTarget) -> Vec<SvgGroupMapping> {
    preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target,
        })
        .collect()
}

fn conversion_options(
    preview: &SvgPreview,
    target: SvgGroupTarget,
    boundary_candidate: Option<SvgBoundaryCandidateId>,
) -> SvgConversionOptions {
    SvgConversionOptions {
        millimetres_per_unit: 1.0,
        group_mappings: mappings(preview, target),
        boundary_candidate,
    }
}

fn candidate(preview: &SvgPreview, kind: SvgBoundaryCandidateKind) -> SvgBoundaryCandidateId {
    preview
        .boundary_candidates()
        .iter()
        .find(|candidate| candidate.kind == kind)
        .expect("boundary candidate kind")
        .id
}

fn has_warning(preview: &SvgPreview, expected: &SvgWarningKind) -> bool {
    preview
        .warnings()
        .iter()
        .any(|warning| &warning.kind == expected)
}

fn assert_approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-10,
        "expected {expected}, found {actual}"
    );
}

#[test]
fn imports_supported_straight_geometry_and_closed_candidates() {
    let preview = preview(
        r#"
                <line x1="1" y1="2" x2="3" y2="4"/>
                <polyline points="10,10 20,10 20,20"/>
                <polyline points="30,30 40,30 40,40 30,30"/>
                <polygon points="50,50 60,50 55,60"/>
                <rect x="65" y="65" width="10" height="5"/>
                <path d="M 80 80 90 80 h 5 v 10 H 80 z"/>
            "#,
    );

    assert_eq!(preview.edges().len(), 1 + 2 + 3 + 3 + 4 + 5);
    assert_eq!(preview.style_groups().len(), 1);
    assert_eq!(
        preview.style_groups()[0].segment_count,
        preview.edges().len()
    );
    let kinds = preview
        .boundary_candidates()
        .iter()
        .map(|candidate| candidate.kind)
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            SvgBoundaryCandidateKind::ViewBox,
            SvgBoundaryCandidateKind::Polyline,
            SvgBoundaryCandidateKind::Polygon,
            SvgBoundaryCandidateKind::Rectangle,
            SvgBoundaryCandidateKind::ClosedPath,
        ]
    );
}

#[test]
fn supports_relative_paths_and_implicit_move_lines() {
    let preview = preview(r#"<path d="m 10 10 10 0 v 10 h -10 z"/>"#);
    assert_eq!(preview.edges().len(), 4);
    let positions = preview
        .edges()
        .iter()
        .flat_map(|edge| edge.vertices)
        .map(|index| preview.vertices()[index].position)
        .map(position_key)
        .collect::<HashSet<_>>();
    assert!(positions.contains(&position_key(Point2::new(10.0, 10.0))));
    assert!(positions.contains(&position_key(Point2::new(20.0, 10.0))));
    assert!(positions.contains(&position_key(Point2::new(20.0, 20.0))));
    assert!(positions.contains(&position_key(Point2::new(10.0, 20.0))));
}

#[test]
fn excludes_curves_and_rounded_rectangles_without_approximation() {
    let preview = preview(
        r#"
                <path d="M 0 0 C 1 1 2 2 3 3 L 4 4"/>
                <rect x="10" y="10" width="20" height="20" rx="2"/>
                <line x1="0" y1="10" x2="10" y2="10"/>
            "#,
    );

    assert_eq!(preview.edges().len(), 1);
    assert_eq!(preview.style_groups().len(), 1);
    assert!(has_warning(
        &preview,
        &SvgWarningKind::UnsupportedPathCommand('C')
    ));
    assert!(has_warning(
        &preview,
        &SvgWarningKind::UnsupportedElement("rounded rect".to_owned())
    ));
}

#[test]
fn rejects_empty_and_curve_only_svg_documents() {
    for body in ["", r#"<path d="M 0 0 C 1 1 2 2 3 3"/>"#] {
        let source = standard_document(body);
        assert!(matches!(
            read_svg_preview(source.as_bytes()),
            Err(SvgImportError::NoSupportedGeometry)
        ));
    }
}

#[test]
fn composes_nested_affine_transforms_without_flipping_y() {
    let preview = preview(
        r#"<g transform="translate(10 20)"><g transform="scale(2)">
                <line x1="1" y1="2" x2="3" y2="4"/>
            </g></g>"#,
    );
    let edge = preview.edges()[0];
    assert_eq!(
        preview.vertices()[edge.vertices[0]].position,
        Point2::new(12.0, 24.0)
    );
    assert_eq!(
        preview.vertices()[edge.vertices[1]].position,
        Point2::new(16.0, 28.0)
    );
}

#[test]
fn supports_rotation_about_a_center() {
    let preview = preview(r#"<line transform="rotate(90 10 0)" x1="10" y1="0" x2="20" y2="0"/>"#);
    let edge = preview.edges()[0];
    let start = preview.vertices()[edge.vertices[0]].position;
    let end = preview.vertices()[edge.vertices[1]].position;
    assert_approx(start.x, 10.0);
    assert_approx(start.y, 0.0);
    assert_approx(end.x, 10.0);
    assert_approx(end.y, 10.0);
}

#[test]
fn svg_transform_model_is_frozen_and_cardinal_rotation_is_bit_exact() {
    let preview = preview(r#"<line transform="rotate(90)" x1="1" y1="0" x2="2" y2="0"/>"#);
    assert_eq!(
        preview.geometry_model_id(),
        "ori_svg_import_geometry_v2__ori_binary64_libm_0_2_16_no_arch_cardinal_v1"
    );
    assert!(
        preview
            .geometry_model_id()
            .ends_with(ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1)
    );
    let edge = preview.edges()[0];
    let endpoint = preview.vertices()[edge.vertices[0]].position;
    assert_eq!(endpoint.x.to_bits(), 0.0_f64.to_bits());
    assert_eq!(endpoint.y.to_bits(), 1.0_f64.to_bits());
}

#[test]
fn rotation_and_skew_are_bit_exact_one_ulp_around_a_cardinal_angle() {
    for angle in [
        f64::from_bits(90.0_f64.to_bits() - 1),
        f64::from_bits(90.0_f64.to_bits() + 1),
    ] {
        let encoded_angle = format!("{angle:.17}");
        assert_eq!(
            encoded_angle
                .parse::<f64>()
                .expect("encoded angle")
                .to_bits(),
            angle.to_bits()
        );
        let rotated = preview(&format!(
            r#"<line transform="rotate({encoded_angle})" x1="1.25" y1="-0.75" x2="2" y2="0"/>"#
        ));
        let edge = rotated.edges()[0];
        let actual = rotated.vertices()[edge.vertices[0]].position;
        let (sine, cosine) =
            ori_numeric::deterministic_sin_cos_degrees_v1(angle).expect("finite angle");
        let expected_x = cosine * 1.25 + (-sine) * -0.75 + 0.0;
        let expected_y = sine * 1.25 + cosine * -0.75 + 0.0;
        assert_eq!(actual.x.to_bits(), expected_x.to_bits());
        assert_eq!(actual.y.to_bits(), expected_y.to_bits());

        let skewed = preview(&format!(
            r#"<line transform="skewX({encoded_angle})" x1="0" y1="1" x2="1" y2="1"/>"#
        ));
        let edge = skewed.edges()[0];
        let actual = skewed.vertices()[edge.vertices[0]].position;
        let expected_tangent = sine / cosine;
        assert!(expected_tangent.is_finite());
        assert_eq!(actual.x.to_bits(), expected_tangent.to_bits());
        assert_eq!(actual.y.to_bits(), 1.0_f64.to_bits());
    }
}

#[test]
fn singular_cardinal_skew_fails_closed() {
    for transform in ["skewX(90)", "skewY(-90)", "skewX(270)", "skewY(-270)"] {
        let source = standard_document(&format!(
            r#"<line transform="{transform}" x1="0" y1="0" x2="1" y2="1"/>"#
        ));
        assert!(matches!(
            read_svg_preview(source.as_bytes()),
            Err(SvgImportError::InvalidTransform)
        ));
    }
}

#[test]
fn deterministic_transform_bits_reach_converted_crease_geometry() {
    let preview =
        preview(r#"<line transform="rotate(90 50 50)" x1="10" y1="20" x2="20" y2="20"/>"#);
    let boundary = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
    let converted = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Mountain,
            Some(boundary),
        ))
        .expect("deterministic transformed geometry converts");
    assert!(
        converted
            .crease_pattern()
            .vertices
            .iter()
            .any(|vertex| vertex.position.x.to_bits() == 80.0_f64.to_bits()
                && vertex.position.y.to_bits() == 10.0_f64.to_bits())
    );
}

#[test]
fn applies_css_inheritance_inline_precedence_and_mapping_hints() {
    let source = document(
        r#"viewBox="0 0 10 10" width="10mm" height="10mm""#,
        r#"
                <style>.fold { stroke: #ff0000; stroke-width: 2; stroke-dasharray: 2 3; }</style>
                <g class="fold" data-origami-layer="creases" data-origami-kind="mountain" opacity="0.5">
                    <line class="chosen" style="stroke: #00ff00" x1="0" y1="0" x2="10" y2="10"/>
                </g>
            "#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("styled SVG");
    let group = &preview.style_groups()[0];

    assert_eq!(
        group.stroke,
        RgbaColor {
            red: 0,
            green: 255,
            blue: 0,
            alpha: 128,
        }
    );
    assert_eq!(group.stroke_width, 2.0);
    assert_eq!(group.dash_pattern, SvgDashPattern::Dashes(vec![2.0, 3.0]));
    assert_eq!(group.classes, vec!["chosen", "fold"]);
    assert_eq!(group.layer.as_deref(), Some("creases"));
    assert_eq!(group.semantic.as_deref(), Some("mountain"));
}

#[test]
fn builds_bounded_layer_paths_without_using_shape_ids_as_layers() {
    let source = document(
        r#"viewBox="0 0 10 10" width="10mm" height="10mm"
               xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape""#,
        r#"
                <g id="outer-id" data-origami-layer="paper" inkscape:label="ignored">
                    <g id="inner-id" inkscape:label="creases" data-layer="ignored">
                        <line id="shape-id" data-origami-layer="shape-ignored"
                              data-fold="mountain" x1="0" y1="0" x2="10" y2="10"/>
                    </g>
                    <g id="identifier-only">
                        <line class="id-fallback" x1="0" y1="10" x2="10" y2="0"/>
                    </g>
                </g>
            "#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("layered SVG");
    let group = &preview.style_groups()[0];

    assert_eq!(group.layer.as_deref(), Some("paper / creases"));
    assert_eq!(group.semantic, None);
    assert!(
        preview
            .style_groups()
            .iter()
            .any(|group| group.layer.as_deref() == Some("paper / identifier-only"))
    );
    assert!(has_warning(
        &preview,
        &SvgWarningKind::UnsupportedAttribute("data-fold".to_owned())
    ));
}

#[test]
fn uses_the_first_retained_shape_id_without_splitting_style_groups() {
    let preview = preview(
        r#"
                <path id="unsupported-first" d="M0 0 C1 1 2 2 3 3"/>
                <line x1="0" y1="20" x2="10" y2="20"/>
                <line id="first-line" x1="0" y1="0" x2="10" y2="0"/>
                <line id="second-line" x1="0" y1="10" x2="10" y2="10"/>
            "#,
    );

    assert_eq!(preview.style_groups().len(), 1);
    assert_eq!(
        preview.style_groups()[0].representative_id.as_deref(),
        Some("first-line")
    );
    assert_eq!(preview.style_groups()[0].segment_count, 3);
    assert_eq!(preview.style_groups()[0].element_count, 3);

    let long_id = format!("\u{0001}{}", "a".repeat(MAX_HINT_CHARS + 20));
    let sanitized = representative_id_hint(Some(&long_id)).expect("sanitized ID");
    assert_eq!(sanitized.chars().count(), MAX_HINT_CHARS);
    assert!(sanitized.chars().all(|character| character == 'a'));
    assert_eq!(representative_id_hint(Some("\n\t")), None);
}

#[test]
fn counts_source_shapes_separately_from_generated_segments() {
    let preview = preview(
        r#"
                <rect x="10" y="10" width="20" height="20"/>
                <polyline points="40,40 50,40 50,50"/>
            "#,
    );
    assert_eq!(preview.style_groups().len(), 1);
    assert_eq!(preview.style_groups()[0].element_count, 2);
    assert_eq!(preview.style_groups()[0].segment_count, 6);
}

#[test]
fn retains_only_canonical_origami_kind_hints() {
    let canonical = [
        "boundary",
        "mountain",
        "valley",
        "auxiliary",
        "cut",
        "ignore",
    ];
    let mut body = String::new();
    for (index, kind) in canonical.iter().enumerate() {
        body.push_str(&format!(
            r#"<line data-origami-kind="{kind}" x1="0" y1="{index}" x2="10" y2="{index}"/>"#
        ));
    }
    body.push_str(
        r#"
                <line data-origami-kind="Mountain" x1="20" y1="0" x2="30" y2="0"/>
                <line data-origami-kind="custom-fold" x1="20" y1="1" x2="30" y2="1"/>
            "#,
    );
    let preview = preview(&body);

    let semantics = preview
        .style_groups()
        .iter()
        .filter_map(|group| group.semantic.clone())
        .collect::<HashSet<_>>();
    assert_eq!(
        semantics,
        canonical.into_iter().map(str::to_owned).collect()
    );
    let unresolved = preview
        .style_groups()
        .iter()
        .find(|group| group.semantic.is_none())
        .expect("non-canonical hints share the no-semantic group");
    assert_eq!(unresolved.element_count, 2);
    let warning = preview
        .warnings()
        .iter()
        .find(|warning| {
            warning.kind == SvgWarningKind::UnsupportedAttribute("data-origami-kind".to_owned())
        })
        .expect("non-canonical semantic warning");
    assert_eq!(warning.occurrences, 2);
}

#[test]
fn resolves_inherited_presentation_class_and_inline_current_color() {
    let styled_preview = preview(
        r##"
                <style>.css-color { color: #445566; stroke: currentColor; }</style>
                <g class="presentation" color="#112233" stroke="currentColor">
                    <line x1="0" y1="10" x2="10" y2="10"/>
                </g>
                <g class="css-color">
                    <line x1="0" y1="20" x2="10" y2="20"/>
                </g>
                <line class="inline" color="#0000ff"
                      style="color: #abcdef; stroke: currentColor"
                      x1="0" y1="30" x2="10" y2="30"/>
            "##,
    );
    let colors = styled_preview
        .style_groups()
        .iter()
        .map(|group| (group.stroke.red, group.stroke.green, group.stroke.blue))
        .collect::<HashSet<_>>();

    assert_eq!(colors.len(), 3);
    assert!(colors.contains(&(0x11, 0x22, 0x33)));
    assert!(colors.contains(&(0x44, 0x55, 0x66)));
    assert!(colors.contains(&(0xab, 0xcd, 0xef)));

    let recursive = standard_document(
        r#"<line stroke="currentColor" color="currentColor" x1="0" y1="0" x2="1" y2="1"/>"#,
    );
    assert!(matches!(
        read_svg_preview(recursive.as_bytes()),
        Err(SvgImportError::NoSupportedGeometry)
    ));
}

#[test]
fn accepts_bounded_stroke_linecaps_through_presentation_and_css_cascade() {
    let preview = preview(
        r#"
                <style>
                    .square { stroke-linecap: square; }
                    .important-round { stroke-linecap: round !important; }
                </style>
                <g stroke-linecap="round">
                    <line x1="0" y1="10" x2="10" y2="10"/>
                    <line stroke-linecap="butt" x1="0" y1="20" x2="10" y2="20"/>
                    <line class="square" x1="0" y1="30" x2="10" y2="30"/>
                    <line class="important-round" style="stroke-linecap: triangle"
                          x1="0" y1="40" x2="10" y2="40"/>
                </g>
            "#,
    );

    assert_eq!(preview.edges().len(), 4);
    assert_eq!(preview.style_groups().len(), 4);
    assert_eq!(
        preview
            .style_groups()
            .iter()
            .map(|group| group.line_cap)
            .collect::<HashSet<_>>(),
        HashSet::from([SvgLineCap::Butt, SvgLineCap::Round, SvgLineCap::Square])
    );
    assert_eq!(
        preview
            .warnings()
            .iter()
            .find(|warning| {
                warning.kind == SvgWarningKind::UnsupportedAttribute("stroke-linecap".to_owned())
            })
            .map(|warning| warning.occurrences),
        Some(1)
    );
}

#[test]
fn linecap_alone_separates_otherwise_identical_source_style_groups() {
    let preview = preview(
        r#"
                <line stroke-linecap="butt" x1="0" y1="10" x2="10" y2="10"/>
                <line stroke-linecap="round" x1="0" y1="20" x2="10" y2="20"/>
                <line stroke-linecap="square" x1="0" y1="30" x2="10" y2="30"/>
            "#,
    );

    assert_eq!(preview.style_groups().len(), 3);
    assert_eq!(
        preview
            .style_groups()
            .iter()
            .map(|group| group.line_cap)
            .collect::<HashSet<_>>(),
        HashSet::from([SvgLineCap::Butt, SvgLineCap::Round, SvgLineCap::Square])
    );
    assert_eq!(
        preview
            .edges()
            .iter()
            .map(|edge| edge.style_group)
            .collect::<HashSet<_>>()
            .len(),
        3
    );
}

#[test]
fn warns_for_unknown_stroke_linecaps_and_rejects_oversized_values() {
    let preview = preview(
        r#"
                <line stroke-linecap="triangle" x1="0" y1="10" x2="10" y2="10"/>
                <line style="stroke-linecap: url(https://example.invalid/external)"
                      x1="0" y1="20" x2="10" y2="20"/>
            "#,
    );
    assert_eq!(preview.edges().len(), 2);
    assert_eq!(
        preview
            .warnings()
            .iter()
            .find(|warning| {
                warning.kind == SvgWarningKind::UnsupportedAttribute("stroke-linecap".to_owned())
            })
            .map(|warning| warning.occurrences),
        Some(2)
    );
    let warning_debug = format!("{:?}", preview.warnings());
    assert!(!warning_debug.contains("triangle"));
    assert!(!warning_debug.contains("example.invalid"));

    let oversized = "x".repeat(MAX_STYLE_VALUE_CHARS + 1);
    let source = standard_document(&format!(
        r#"<line stroke-linecap="{oversized}" x1="0" y1="0" x2="10" y2="10"/>"#
    ));
    assert!(matches!(
        read_svg_preview(source.as_bytes()),
        Err(SvgImportError::StyleValueTooLong {
            property,
            maximum: MAX_STYLE_VALUE_CHARS
        }) if property == "stroke-linecap"
    ));
}

#[test]
fn invalid_linecap_declarations_warn_and_fall_back_through_the_css_cascade() {
    let preview = preview(
        r#"
                <g stroke-linecap="round">
                    <line stroke-linecap="triangle"
                          x1="0" y1="10" x2="10" y2="10"/>
                    <line x1="0" y1="20" x2="10" y2="20"/>
                </g>
                <line stroke-linecap="square" style="stroke-linecap: triangle"
                      x1="0" y1="30" x2="10" y2="30"/>
            "#,
    );

    assert_eq!(preview.style_groups().len(), 2);
    assert_eq!(
        preview
            .style_groups()
            .iter()
            .map(|group| group.line_cap)
            .collect::<HashSet<_>>(),
        HashSet::from([SvgLineCap::Round, SvgLineCap::Square])
    );
    assert_eq!(
        preview
            .style_groups()
            .iter()
            .find(|group| group.line_cap == SvgLineCap::Round)
            .map(|group| group.segment_count),
        Some(2)
    );
    assert_eq!(
        preview
            .warnings()
            .iter()
            .find(|warning| {
                warning.kind == SvgWarningKind::UnsupportedAttribute("stroke-linecap".to_owned())
            })
            .map(|warning| warning.occurrences),
        Some(2)
    );
}

#[test]
fn recommends_physical_scale_only_from_valid_root_geometry() {
    let source = document(
        r#"viewBox="0 0 210 297" width="21cm" height="297mm""#,
        r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("physical SVG");
    assert_eq!(preview.recommended_millimetres_per_unit(), Some(1.0));
    assert_eq!(
        preview.root_view_box(),
        Some(SvgRootViewBox {
            x: 0.0,
            y: 0.0,
            width: 210.0,
            height: 297.0,
        })
    );
    assert_eq!(
        preview.root_physical_size(),
        SvgRootPhysicalSize {
            width_millimetres: Some(210.0),
            height_millimetres: Some(297.0),
            width_unit: Some(SvgRootLengthUnit::Cm),
            height_unit: Some(SvgRootLengthUnit::Mm),
        }
    );
    assert!(!has_warning(
        &preview,
        &SvgWarningKind::CssPixelScaleAssumed
    ));

    let source = document(
        r#"viewBox="0 0 100 100" width="100mm" height="200mm""#,
        r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("non-uniform SVG");
    assert_eq!(preview.recommended_millimetres_per_unit(), None);
    assert!(has_warning(
        &preview,
        &SvgWarningKind::PhysicalScaleNeedsSelection
    ));

    let source = document(
        r#"viewBox="0 0 1 1" width="0.000000001mm" height="0.0000000010005mm""#,
        r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("tiny non-uniform SVG");
    assert_eq!(preview.recommended_millimetres_per_unit(), None);

    let source = document(
        r#"viewBox="0 0 1 1" width="1e308mm" height="1e308mm""#,
        r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("huge uniform SVG");
    assert_eq!(preview.recommended_millimetres_per_unit(), Some(1e308));
}

#[test]
fn reports_when_root_dimensions_use_css_pixels() {
    let source = document(
        r#"viewBox="10 20 96 192" width="96px" height="192""#,
        r#"<line x1="10" y1="20" x2="11" y2="21"/>"#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("CSS pixel SVG");
    assert_approx(
        preview
            .recommended_millimetres_per_unit()
            .expect("automatic scale"),
        25.4 / 96.0,
    );
    assert_eq!(
        preview.root_view_box(),
        Some(SvgRootViewBox {
            x: 10.0,
            y: 20.0,
            width: 96.0,
            height: 192.0,
        })
    );
    let physical_size = preview.root_physical_size();
    assert_approx(
        physical_size.width_millimetres.expect("physical width"),
        25.4,
    );
    assert_approx(
        physical_size.height_millimetres.expect("physical height"),
        50.8,
    );
    assert_eq!(physical_size.width_unit, Some(SvgRootLengthUnit::Px));
    assert_eq!(physical_size.height_unit, Some(SvgRootLengthUnit::Unitless));
    assert!(has_warning(&preview, &SvgWarningKind::CssPixelScaleAssumed));

    let source = document(
        r#"viewBox="0 0 96 96" width="96px" height="192px""#,
        r#"<line x1="0" y1="0" x2="1" y2="1"/>"#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("non-uniform CSS pixel SVG");
    assert_eq!(preview.recommended_millimetres_per_unit(), None);
    assert!(has_warning(
        &preview,
        &SvgWarningKind::PhysicalScaleNeedsSelection
    ));
    assert!(has_warning(&preview, &SvgWarningKind::CssPixelScaleAssumed));
}

#[test]
fn accepts_trimmed_absolute_lengths_and_q_units() {
    let inch = parse_root_physical_length(" 1in ").unwrap();
    assert_approx(inch.millimetres.unwrap(), 25.4);
    assert_eq!(inch.unit, SvgRootLengthUnit::In);
    assert!(!inch.css_pixels_assumed);
    let q = parse_root_physical_length(" 4Q ").unwrap();
    assert_approx(q.millimetres.unwrap(), 1.0);
    assert_eq!(q.unit, SvgRootLengthUnit::Q);
    assert!(!q.css_pixels_assumed);
    let pixels = parse_root_physical_length(" 96px ").unwrap();
    assert_approx(pixels.millimetres.unwrap(), 25.4);
    assert_eq!(pixels.unit, SvgRootLengthUnit::Px);
    assert!(pixels.css_pixels_assumed);
    let unitless = parse_root_physical_length("96").unwrap();
    assert_eq!(unitless.unit, SvgRootLengthUnit::Unitless);
    assert!(unitless.css_pixels_assumed);
    let relative = parse_root_physical_length("2em").unwrap();
    assert_eq!(relative.millimetres, None);
    assert_eq!(relative.unit, SvgRootLengthUnit::Em);
    assert!(parse_root_physical_length("1e309Q").is_err());
    assert_approx(parse_supported_length(" 25.4mm ").unwrap(), 96.0);
}

#[test]
fn source_candidate_overrides_group_mapping_and_preserves_correspondence() {
    let preview = preview(r#"<rect x="10" y="10" width="80" height="80"/>"#);
    let rectangle = candidate(&preview, SvgBoundaryCandidateKind::Rectangle);
    let converted = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Mountain,
            Some(rectangle),
        ))
        .expect("rectangle conversion");

    assert_eq!(converted.boundary_vertices().len(), 4);
    assert_eq!(converted.crease_pattern().edges.len(), 4);
    assert!(
        converted
            .crease_pattern()
            .edges
            .iter()
            .all(|edge| edge.kind == EdgeKind::Boundary)
    );
    assert_eq!(converted.groups()[0].target, SvgGroupTarget::Mountain);
    assert_eq!(converted.groups()[0].edge_ids.len(), 4);
}

#[test]
fn rejects_candidate_and_boundary_group_combination() {
    let preview = preview(r#"<rect x="10" y="10" width="80" height="80"/>"#);
    let rectangle = candidate(&preview, SvgBoundaryCandidateKind::Rectangle);
    let error = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Boundary,
            Some(rectangle),
        ))
        .expect_err("candidate conflict");
    assert_eq!(error, SvgConversionError::BoundaryCandidateMappingConflict);
}

#[test]
fn view_box_candidate_planarizes_x_crossings_and_boundary_touches() {
    let preview = preview(
        r#"
                <line x1="0" y1="50" x2="100" y2="50"/>
                <line x1="50" y1="0" x2="50" y2="100"/>
            "#,
    );
    let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
    let converted = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Mountain,
            Some(view_box),
        ))
        .expect("planarized crossing");

    assert_eq!(converted.crease_pattern().vertices.len(), 9);
    assert_eq!(converted.crease_pattern().edges.len(), 12);
    assert_eq!(
        converted
            .crease_pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
            .count(),
        8
    );
    assert_eq!(converted.groups()[0].edge_ids.len(), 4);
    assert!(validate_crease_pattern(converted.crease_pattern()).is_valid());
}

#[test]
fn planarizes_t_junctions() {
    let preview = preview(
        r#"
                <line x1="0" y1="50" x2="100" y2="50"/>
                <line x1="50" y1="0" x2="50" y2="50"/>
            "#,
    );
    let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
    let converted = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Valley,
            Some(view_box),
        ))
        .expect("T junction");

    assert_eq!(converted.groups()[0].edge_ids.len(), 3);
    assert!(validate_crease_pattern(converted.crease_pattern()).is_valid());
}

#[test]
fn rejects_collinear_overlap_instead_of_guessing() {
    let preview = preview(r#"<line x1="0" y1="0" x2="100" y2="0"/>"#);
    let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
    let error = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Auxiliary,
            Some(view_box),
        ))
        .expect_err("overlapping boundary");
    assert!(matches!(error, SvgConversionError::CollinearOverlap { .. }));
}

#[test]
fn maps_source_boundary_without_selecting_a_candidate() {
    let preview = preview(r#"<rect x="10" y="10" width="80" height="80"/>"#);
    let converted = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Boundary,
            None,
        ))
        .expect("mapped boundary");
    assert_eq!(converted.boundary_vertices().len(), 4);
    assert!(validate_crease_pattern(converted.crease_pattern()).is_valid());
}

#[test]
fn rejects_disconnected_boundary_cycles() {
    let preview = preview(
        r#"
                <rect x="10" y="10" width="20" height="20"/>
                <rect x="60" y="60" width="20" height="20"/>
            "#,
    );
    let error = preview
        .convert(&conversion_options(
            &preview,
            SvgGroupTarget::Boundary,
            None,
        ))
        .expect_err("disconnected boundaries");
    assert_eq!(error, SvgConversionError::BoundaryDisconnected);
}

#[test]
fn reports_cut_edges_and_ignored_groups() {
    let source = standard_document(
        r#"
                <line class="cut" x1="10" y1="10" x2="90" y2="90"/>
                <line class="guide" x1="10" y1="90" x2="90" y2="10"/>
            "#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("cut preview");
    assert_eq!(preview.style_groups().len(), 2);
    let options = SvgConversionOptions {
        millimetres_per_unit: 1.0,
        group_mappings: vec![
            SvgGroupMapping {
                group: preview.style_groups()[0].id,
                target: SvgGroupTarget::Cut,
            },
            SvgGroupMapping {
                group: preview.style_groups()[1].id,
                target: SvgGroupTarget::Ignore,
            },
        ],
        boundary_candidate: Some(candidate(&preview, SvgBoundaryCandidateKind::ViewBox)),
    };
    let converted = preview.convert(&options).expect("cut conversion");

    assert!(converted.has_cuts());
    assert_eq!(converted.groups()[0].edge_ids.len(), 1);
    assert!(converted.groups()[1].edge_ids.is_empty());
}

#[test]
fn requires_exactly_one_mapping_for_every_group() {
    let preview = preview(r#"<line x1="10" y1="10" x2="90" y2="90"/>"#);
    let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);

    let missing = SvgConversionOptions {
        millimetres_per_unit: 1.0,
        group_mappings: Vec::new(),
        boundary_candidate: Some(view_box),
    };
    assert_eq!(
        preview.convert(&missing).expect_err("missing mapping"),
        SvgConversionError::MissingGroupMapping {
            group: preview.style_groups()[0].id,
        }
    );

    let group = preview.style_groups()[0].id;
    let duplicate = SvgConversionOptions {
        millimetres_per_unit: 1.0,
        group_mappings: vec![
            SvgGroupMapping {
                group,
                target: SvgGroupTarget::Mountain,
            },
            SvgGroupMapping {
                group,
                target: SvgGroupTarget::Valley,
            },
        ],
        boundary_candidate: Some(view_box),
    };
    assert_eq!(
        preview.convert(&duplicate).expect_err("duplicate mapping"),
        SvgConversionError::DuplicateGroupMapping { group }
    );

    let unknown_group = SvgStyleGroupId(999);
    let unknown = SvgConversionOptions {
        millimetres_per_unit: 1.0,
        group_mappings: vec![SvgGroupMapping {
            group: unknown_group,
            target: SvgGroupTarget::Mountain,
        }],
        boundary_candidate: Some(view_box),
    };
    assert_eq!(
        preview.convert(&unknown).expect_err("unknown mapping"),
        SvgConversionError::UnknownGroupMapping {
            group: unknown_group,
        }
    );
}

#[test]
fn validates_scale_and_boundary_candidate_ids() {
    let preview = preview(r#"<line x1="10" y1="10" x2="90" y2="90"/>"#);
    let options = SvgConversionOptions {
        millimetres_per_unit: 0.0,
        group_mappings: mappings(&preview, SvgGroupTarget::Mountain),
        boundary_candidate: None,
    };
    assert_eq!(
        preview.convert(&options).expect_err("zero scale"),
        SvgConversionError::InvalidMillimetresPerUnit
    );

    let options = SvgConversionOptions {
        millimetres_per_unit: 1.0,
        group_mappings: mappings(&preview, SvgGroupTarget::Mountain),
        boundary_candidate: Some(SvgBoundaryCandidateId(999)),
    };
    assert_eq!(
        preview.convert(&options).expect_err("unknown candidate"),
        SvgConversionError::UnknownBoundaryCandidate {
            candidate: SvgBoundaryCandidateId(999),
        }
    );
}

#[test]
fn deduplicates_exact_source_endpoints() {
    let source = document(
        "",
        r#"
                <line x1="0" y1="0" x2="10" y2="0"/>
                <line x1="10" y1="0" x2="10" y2="10"/>
            "#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("joined lines");
    assert_eq!(preview.vertices().len(), 3);
    assert_eq!(
        preview.edges()[0].vertices[1],
        preview.edges()[1].vertices[0]
    );
}

#[test]
fn enforces_svg_namespace_for_root_and_descendants() {
    let missing = br##"<svg stroke="#000"><line x2="10"/></svg>"##;
    assert!(matches!(
        read_svg_preview(missing),
        Err(SvgImportError::InvalidSvgNamespace)
    ));

    let source = standard_document(
        r#"
                <g xmlns="urn:not-svg"><line x1="0" y1="0" x2="10" y2="10"/></g>
                <line x1="10" y1="10" x2="20" y2="20"/>
            "#,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("namespace switch");
    assert_eq!(preview.edges().len(), 1);
    assert!(has_warning(
        &preview,
        &SvgWarningKind::UnsupportedElement("g".to_owned())
    ));
}

#[test]
fn ignores_stylesheets_from_other_namespaces() {
    let source = document(
        r#"xmlns:x="urn:not-svg" viewBox="0 0 10 10" width="10mm" height="10mm""#,
        r##"
                <x:style>.fold { stroke: red; }</x:style>
                <line class="fold" stroke="none" x1="0" y1="0" x2="10" y2="10"/>
                <line stroke="#000" x1="0" y1="10" x2="10" y2="0"/>
            "##,
    );
    let preview = read_svg_preview(source.as_bytes()).expect("foreign stylesheet");
    assert_eq!(preview.edges().len(), 1);
}

#[test]
fn rejects_undeclared_prefixes_doctypes_and_non_utf8() {
    let undeclared = br#"<svg xmlns="http://www.w3.org/2000/svg"><x:line x2="1"/></svg>"#;
    assert!(matches!(
        read_svg_preview(undeclared),
        Err(SvgImportError::InvalidXml(_))
    ));

    let doctype = br#"<!DOCTYPE svg [<!ENTITY x "1">]><svg xmlns="http://www.w3.org/2000/svg"/>"#;
    assert!(matches!(
        read_svg_preview(doctype),
        Err(SvgImportError::DoctypeNotAllowed)
    ));

    assert!(matches!(
        read_svg_preview(&[0xff, 0xfe, 0x00]),
        Err(SvgImportError::NonUtf8)
    ));
}

#[test]
fn rejects_invalid_xml_declarations_and_trailing_content() {
    let encoding =
        br#"<?xml version="1.0" encoding="UTF-16"?><svg xmlns="http://www.w3.org/2000/svg"/>"#;
    assert!(matches!(
        read_svg_preview(encoding),
        Err(SvgImportError::UnsupportedXmlDeclaration)
    ));

    let duplicate = br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"><?xml version="1.0"?></svg>"#;
    assert!(read_svg_preview(duplicate).is_err());

    let before = br#"not-xml<svg xmlns="http://www.w3.org/2000/svg"/>"#;
    assert!(matches!(
        read_svg_preview(before),
        Err(SvgImportError::TrailingContent)
    ));
    let after = br#"<svg xmlns="http://www.w3.org/2000/svg"/>not-xml"#;
    assert!(matches!(
        read_svg_preview(after),
        Err(SvgImportError::TrailingContent)
    ));
}

#[test]
fn rejects_trailing_garbage_in_strict_numeric_attributes() {
    let view_box = document(r#"viewBox="0 0 10 10 junk" width="10mm" height="10mm""#, "");
    assert!(matches!(
        read_svg_preview(view_box.as_bytes()),
        Err(SvgImportError::InvalidAttribute { attribute, .. }) if attribute == "viewBox"
    ));

    let aspect = document(
        r#"viewBox="0 0 10 10" width="10mm" height="10mm" preserveAspectRatio="xMidYMid slice junk""#,
        "",
    );
    assert!(matches!(
        read_svg_preview(aspect.as_bytes()),
        Err(SvgImportError::InvalidAttribute { attribute, .. })
            if attribute == "preserveAspectRatio"
    ));

    let points = standard_document(r#"<polyline points="0,0 1,1,"/>"#);
    assert!(matches!(
        read_svg_preview(points.as_bytes()),
        Err(SvgImportError::InvalidAttribute { attribute, .. }) if attribute == "points"
    ));
}

#[test]
fn reports_unsupported_external_and_hidden_content() {
    let preview = preview(
        r#"
                <image href="https://example.invalid/a.png"/>
                <circle cx="1" cy="1" r="1"/>
                <line display="none" x1="0" y1="0" x2="1" y2="1"/>
                <line x1="0" y1="1" x2="1" y2="0"/>
            "#,
    );
    assert!(has_warning(
        &preview,
        &SvgWarningKind::ExternalReferenceIgnored
    ));
    assert!(has_warning(
        &preview,
        &SvgWarningKind::UnsupportedElement("image".to_owned())
    ));
    assert!(has_warning(
        &preview,
        &SvgWarningKind::UnsupportedElement("circle".to_owned())
    ));
    assert!(has_warning(
        &preview,
        &SvgWarningKind::HiddenGeometryIgnored
    ));
}

#[test]
fn enforces_parser_resource_limits() {
    let source = standard_document(r#"<g><g><line x2="1"/></g></g>"#);
    let limits = SvgImportLimits {
        max_depth: 2,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooDeep { .. })
    ));

    let source = standard_document(r#"<line x1="0" y1="0" x2="1" y2="1"/>"#);
    let limits = SvgImportLimits {
        max_elements: 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManyElements { .. })
    ));

    let limits = SvgImportLimits {
        max_attributes_per_element: 2,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::InvalidXml(_)) | Err(SvgImportError::TooManyAttributes { .. })
    ));

    let limits = SvgImportLimits {
        max_file_bytes: source.len() - 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::FileTooLarge { .. })
    ));
}

#[test]
fn enforces_geometry_css_warning_and_candidate_limits() {
    let source = standard_document(r#"<line x2="1"/><line x1="1" x2="2"/>"#);
    let limits = SvgImportLimits {
        max_source_edges: 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManySourceEdges { .. })
    ));

    let source = standard_document(r#"<path d="M0 0 L1 0 L2 0"/>"#);
    let limits = SvgImportLimits {
        max_path_commands: 2,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManyPathCommands { .. })
    ));

    let source = standard_document(
        r#"<style>.a{stroke:red}.b{stroke:blue}</style><line class="a" x2="1"/>"#,
    );
    let limits = SvgImportLimits {
        max_css_rules: 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManyCssRules { .. })
    ));

    let source = standard_document(r#"<line class="a" x2="1"/><line class="b" x1="1" x2="2"/>"#);
    let limits = SvgImportLimits {
        max_style_groups: 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManyStyleGroups { .. })
    ));

    let source = standard_document(r#"<rect x="10" y="10" width="20" height="20"/>"#);
    let limits = SvgImportLimits {
        max_boundary_candidates: 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManyBoundaryCandidates { .. })
    ));

    let source = standard_document(r#"<circle/><ellipse/>"#);
    let limits = SvgImportLimits {
        max_warnings: 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManyWarnings { .. })
    ));
}

#[test]
fn bounds_css_rule_element_evaluation_work_at_the_exact_limit() {
    let source =
        standard_document(r#"<style>.fold { stroke: red; }</style><line class="fold" x2="1"/>"#);
    let exact = SvgImportLimits {
        max_css_rule_element_evaluations: 3,
        ..SvgImportLimits::default()
    };
    let preview =
        read_svg_preview_with_limits(source.as_bytes(), exact).expect("exact CSS work limit");
    assert_eq!(preview.edges().len(), 1);

    let one_too_many = SvgImportLimits {
        max_css_rule_element_evaluations: 2,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), one_too_many),
        Err(SvgImportError::TooManyCssRuleElementEvaluations { maximum: 2 })
    ));
}

#[test]
fn bounds_individual_css_selectors_and_property_values() {
    let exact_value = format!("{}1", "0".repeat(MAX_STYLE_VALUE_CHARS - 1));
    let source = standard_document(&format!(
        r#"<line style="stroke-width:{exact_value}" x2="1"/>"#
    ));
    read_svg_preview(source.as_bytes()).expect("exact supported style value limit");

    let oversized_value = format!("{}1", "0".repeat(MAX_STYLE_VALUE_CHARS));
    let source = standard_document(&format!(
        r#"<line style="stroke-width:{oversized_value}" x2="1"/>"#
    ));
    assert!(matches!(
        read_svg_preview(source.as_bytes()),
        Err(SvgImportError::StyleValueTooLong { maximum: 120, .. })
    ));

    let exact_selector = "a".repeat(MAX_CSS_SELECTOR_CHARS);
    let source = standard_document(&format!(
        r#"<style>{exact_selector} {{ stroke: red; }}</style><line x2="1"/>"#
    ));
    read_svg_preview(source.as_bytes()).expect("exact selector limit");

    let oversized_selector = "a".repeat(MAX_CSS_SELECTOR_CHARS + 1);
    let source = standard_document(&format!(
        r#"<style>{oversized_selector} {{ stroke: red; }}</style><line x2="1"/>"#
    ));
    assert!(matches!(
        read_svg_preview(source.as_bytes()),
        Err(SvgImportError::CssSelectorTooLong { maximum: 120 })
    ));
}

#[test]
fn ignores_bounded_unsupported_style_declarations_without_rejecting_geometry() {
    let oversized_unsupported_value = "x".repeat(MAX_STYLE_VALUE_CHARS + 1);
    let unsupported = (0..40)
        .map(|index| format!("vendor-property-{index}:{oversized_unsupported_value}"))
        .collect::<Vec<_>>()
        .join(";");
    let source = standard_document(&format!(
        r#"<line style="{unsupported};stroke:#ff0000 !important" x1="10" y1="10" x2="90" y2="90"/>"#
    ));

    let preview = read_svg_preview(source.as_bytes())
        .expect("bounded unsupported declarations must be ignored");

    assert_eq!(preview.edges().len(), 1);
    assert_eq!(
        preview.style_groups()[0].stroke,
        RgbaColor::opaque(255, 0, 0)
    );
    for index in 0..40 {
        assert!(has_warning(
            &preview,
            &SvgWarningKind::UnsupportedStyleProperty(format!("vendor-property-{index}"))
        ));
    }
}

#[test]
fn unsupported_styles_remain_bounded_by_the_document_style_text_limit() {
    let oversized_style = format!("vendor-property:{}", "x".repeat(MAX_STYLE_TEXT_BYTES));
    let source = standard_document(&format!(
        r#"<line style="{oversized_style}" x1="10" y1="10" x2="90" y2="90"/>"#
    ));

    assert!(matches!(
        read_svg_preview(source.as_bytes()),
        Err(SvgImportError::InvalidCss)
    ));
}

#[test]
fn important_declarations_follow_css_cascade_instead_of_rejecting_the_document() {
    let source = standard_document(
        r#"
                <style>
                    .fold { stroke: #ff0000 !important; }
                    .fold { stroke: #0000ff; }
                    .same-priority { stroke: #0000ff !important; }
                    .same-priority { stroke: #ffff00 !important; }
                </style>
                <line class="fold" style="stroke:#00ff00" x1="10" y1="10" x2="90" y2="90"/>
                <line class="fold" style="stroke:#00ff00 !important" x1="10" y1="90" x2="90" y2="10"/>
                <line class="same-priority" x1="10" y1="50" x2="90" y2="50"/>
            "#,
    );

    let preview = read_svg_preview(source.as_bytes()).expect("important CSS");

    assert_eq!(preview.edges().len(), 3);
    assert_eq!(preview.style_groups().len(), 3);
    assert_eq!(
        preview.style_groups()[0].stroke,
        RgbaColor::opaque(255, 0, 0),
        "stylesheet !important must beat a normal inline declaration"
    );
    assert_eq!(
        preview.style_groups()[1].stroke,
        RgbaColor::opaque(0, 255, 0),
        "inline !important must beat stylesheet !important"
    );
    assert_eq!(
        preview.style_groups()[2].stroke,
        RgbaColor::opaque(255, 255, 0),
        "a later declaration must win at equal specificity and importance"
    );
}

#[test]
fn css_property_names_are_ascii_case_insensitive() {
    let source = standard_document(
        r#"
                <line
                    style="STROKE:#ff0000 !IMPORTANT;Stroke-Width:2"
                    x1="10"
                    y1="10"
                    x2="90"
                    y2="90"
                />
            "#,
    );

    let preview =
        read_svg_preview(source.as_bytes()).expect("ASCII case-insensitive CSS properties");

    assert_eq!(preview.edges().len(), 1);
    assert_eq!(preview.style_groups().len(), 1);
    assert_eq!(
        preview.style_groups()[0].stroke,
        RgbaColor::opaque(255, 0, 0)
    );
    assert_eq!(preview.style_groups()[0].stroke_width, 2.0);
    assert!(
        !preview
            .warnings()
            .iter()
            .any(|warning| matches!(warning.kind, SvgWarningKind::UnsupportedStyleProperty(_))),
        "supported CSS property spelling must not be downgraded to an unsupported warning"
    );
}

#[test]
fn enforces_intersection_and_final_geometry_limits() {
    let source = standard_document("");
    let limits = SvgImportLimits {
        max_intersection_candidates: 1,
        ..SvgImportLimits::default()
    };
    assert!(matches!(
        read_svg_preview_with_limits(source.as_bytes(), limits),
        Err(SvgImportError::TooManyBoundaryCandidateIntersections { .. })
    ));

    let source = document(
        "",
        r#"
                <line class="boundary" x1="0" y1="0" x2="10" y2="0"/>
                <line class="boundary" x1="10" y1="0" x2="10" y2="10"/>
                <line class="boundary" x1="10" y1="10" x2="0" y2="10"/>
                <line class="boundary" x1="0" y1="10" x2="0" y2="0"/>
                <line class="crease" x1="0" y1="5" x2="10" y2="5"/>
            "#,
    );
    let limits = SvgImportLimits {
        max_intersection_candidates: 0,
        ..SvgImportLimits::default()
    };
    let limited_preview =
        read_svg_preview_with_limits(source.as_bytes(), limits).expect("bounded preview");
    let mut mappings = limited_preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: if group.classes == ["boundary"] {
                SvgGroupTarget::Boundary
            } else {
                SvgGroupTarget::Mountain
            },
        })
        .collect::<Vec<_>>();
    mappings.sort_by_key(|mapping| mapping.group);
    let options = SvgConversionOptions {
        millimetres_per_unit: 1.0,
        group_mappings: mappings,
        boundary_candidate: None,
    };
    assert_eq!(
        limited_preview
            .convert(&options)
            .expect_err("intersection cap"),
        SvgConversionError::TooManyIntersectionCandidates { maximum: 0 }
    );

    let preview = preview(r#"<line x1="10" y1="10" x2="20" y2="20"/>"#);
    let view_box = candidate(&preview, SvgBoundaryCandidateKind::ViewBox);
    let mut constrained = preview.clone();
    constrained.limits.max_final_edges = 3;
    assert_eq!(
        constrained
            .convert(&SvgConversionOptions {
                millimetres_per_unit: 1.0,
                group_mappings: constrained
                    .style_groups()
                    .iter()
                    .map(|group| SvgGroupMapping {
                        group: group.id,
                        target: SvgGroupTarget::Ignore,
                    })
                    .collect(),
                boundary_candidate: Some(view_box),
            })
            .expect_err("edge cap"),
        SvgConversionError::TooManyFinalEdges { maximum: 3 }
    );
}
