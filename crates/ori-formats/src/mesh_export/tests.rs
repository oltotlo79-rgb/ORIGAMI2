use super::*;

fn sample_document() -> IndexedTriangleMeshV1 {
    IndexedTriangleMeshV1::new(
        "折り紙 #1",
        vec![
            [-0.0, 0.0, 0.0],
            [100.0, -0.0, 0.0],
            [100.0, 50.0, 0.0],
            [0.0, 50.0, 0.0],
        ],
        vec![[0.0, 0.0, 2.0]; 4],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

#[test]
fn glb_embeds_bounded_png_texture_and_uvs_for_independent_reader() {
    // The image reader is deliberately not invoked by this interchange
    // test; these bytes are a complete 1x1 RGBA PNG kept in source only.
    let png = vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31,
        0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let document = sample_document().with_base_color_texture(EmbeddedBaseColorTextureV1 {
        media_type: EmbeddedTextureMediaTypeV1::Png,
        bytes: png.clone(),
        tex_coords: uvs.clone(),
    });
    let mesh = validate_indexed_triangle_mesh(&document).unwrap();
    let artifact = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).unwrap();
    let gltf = gltf::Gltf::from_slice(&artifact.bytes).expect("independent glTF reader");
    let blob = gltf.blob.as_deref().unwrap();
    let primitive = gltf.meshes().next().unwrap().primitives().next().unwrap();
    let read_uvs: Vec<_> = primitive
        .reader(|_| Some(blob))
        .read_tex_coords(0)
        .unwrap()
        .into_f32()
        .collect();
    assert_eq!(read_uvs, uvs);
    let material = primitive.material();
    assert_eq!(
        material
            .pbr_metallic_roughness()
            .base_color_texture()
            .unwrap()
            .texture()
            .index(),
        0
    );
    let image = gltf.images().next().unwrap();
    let gltf::image::Source::View { view, mime_type } = image.source() else {
        panic!("embedded image required")
    };
    assert_eq!(mime_type, "image/png");
    assert_eq!(&blob[view.offset()..view.offset() + view.length()], png);
}

#[test]
fn dual_sided_glb_has_independent_front_and_back_primitives_materials_and_images() {
    let png = vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31,
        0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let front_uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let back_uvs = vec![[1.0, 0.0], [0.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
    let document = sample_document().with_base_color_texture(EmbeddedBaseColorTextureV1 {
        media_type: EmbeddedTextureMediaTypeV1::Png,
        bytes: png.clone(),
        tex_coords: front_uvs.clone(),
    });
    let mesh = validate_indexed_triangle_mesh(&document).unwrap();
    let back = EmbeddedBaseColorTextureV1 {
        media_type: EmbeddedTextureMediaTypeV1::Png,
        bytes: png.clone(),
        tex_coords: back_uvs.clone(),
    };
    let artifact =
        export_dual_sided_triangle_mesh_glb(&mesh, back.clone(), [20, 30, 40, 255]).unwrap();
    let gltf = gltf::Gltf::from_slice(&artifact.bytes).expect("independent glTF reader");
    let blob = gltf.blob.as_deref().unwrap();
    let primitives = gltf
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .collect::<Vec<_>>();
    assert_eq!(primitives.len(), 2);
    assert_eq!(gltf.materials().count(), 2);
    assert_eq!(gltf.images().count(), 2);
    assert_eq!(gltf.textures().count(), 2);
    let front_indices = primitives[0]
        .reader(|_| Some(blob))
        .read_indices()
        .unwrap()
        .into_u32()
        .collect::<Vec<_>>();
    let back_indices = primitives[1]
        .reader(|_| Some(blob))
        .read_indices()
        .unwrap()
        .into_u32()
        .collect::<Vec<_>>();
    assert_eq!(front_indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(back_indices, vec![0, 2, 1, 0, 3, 2]);
    let front_normals = primitives[0]
        .reader(|_| Some(blob))
        .read_normals()
        .unwrap()
        .collect::<Vec<_>>();
    let back_normals = primitives[1]
        .reader(|_| Some(blob))
        .read_normals()
        .unwrap()
        .collect::<Vec<_>>();
    assert!(front_normals.iter().all(|normal| normal[2] == 1.0));
    assert!(back_normals.iter().all(|normal| normal[2] == -1.0));
    let read_back_uvs = primitives[1]
        .reader(|_| Some(blob))
        .read_tex_coords(0)
        .unwrap()
        .into_f32()
        .collect::<Vec<_>>();
    assert_eq!(read_back_uvs, back_uvs);
    assert_eq!(primitives[0].material().index(), Some(0));
    assert_eq!(primitives[1].material().index(), Some(1));
    for image in gltf.images() {
        let gltf::image::Source::View { view, mime_type } = image.source() else {
            panic!("embedded image required")
        };
        assert_eq!(mime_type, "image/png");
        assert_eq!(&blob[view.offset()..view.offset() + view.length()], png);
    }
    assert_eq!(artifact.triangle_count, 4);

    let mut invalid = back.clone();
    invalid.tex_coords.pop();
    assert!(matches!(
        export_dual_sided_triangle_mesh_glb(&mesh, invalid, [0; 4]),
        Err(StaticMeshExportError::TextureCoordinateCountMismatch { .. })
    ));
    let exact = StaticMeshExportLimits {
        max_output_bytes: artifact.bytes.len(),
        ..StaticMeshExportLimits::default()
    };
    assert!(
        export_dual_sided_triangle_mesh_glb_with_limits(
            &mesh,
            back.clone(),
            [20, 30, 40, 255],
            exact,
        )
        .is_ok()
    );
    let one_short = StaticMeshExportLimits {
        max_output_bytes: artifact.bytes.len() - 1,
        ..StaticMeshExportLimits::default()
    };
    assert!(matches!(
        export_dual_sided_triangle_mesh_glb_with_limits(&mesh, back, [20, 30, 40, 255], one_short),
        Err(StaticMeshExportError::OutputTooLarge { .. })
    ));
}

#[test]
fn closed_solid_glb_regions_are_complete_and_side_wall_is_untextured() {
    let png = vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31,
        0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let uvs = vec![[0.0, 0.0]; 6];
    let document = IndexedTriangleMeshV1::new(
        "closed prism",
        vec![
            [0.0, 0.0, 1.0],
            [10.0, 0.0, 1.0],
            [0.0, 10.0, 1.0],
            [0.0, 0.0, -1.0],
            [10.0, 0.0, -1.0],
            [0.0, 10.0, -1.0],
        ],
        vec![[0.0, 0.0, 1.0]; 6],
        vec![
            [0, 1, 2],
            [3, 5, 4],
            [0, 3, 4],
            [0, 4, 1],
            [1, 4, 5],
            [1, 5, 2],
            [2, 5, 3],
            [2, 3, 0],
        ],
    )
    .with_base_color_texture(EmbeddedBaseColorTextureV1 {
        media_type: EmbeddedTextureMediaTypeV1::Png,
        bytes: png.clone(),
        tex_coords: uvs.clone(),
    });
    let mesh = validate_indexed_triangle_mesh(&document).unwrap();
    let regions = vec![
        ClosedSolidTriangleRegionV1::FrontCap,
        ClosedSolidTriangleRegionV1::BackCap,
        ClosedSolidTriangleRegionV1::SideWall,
        ClosedSolidTriangleRegionV1::SideWall,
        ClosedSolidTriangleRegionV1::SideWall,
        ClosedSolidTriangleRegionV1::SideWall,
        ClosedSolidTriangleRegionV1::SideWall,
        ClosedSolidTriangleRegionV1::SideWall,
    ];
    let artifact = export_regioned_closed_solid_triangle_mesh_glb(
        &mesh,
        &regions,
        EmbeddedBaseColorTextureV1 {
            media_type: EmbeddedTextureMediaTypeV1::Png,
            bytes: png.clone(),
            tex_coords: uvs,
        },
        [10, 20, 30, 255],
        [80, 70, 60, 255],
    )
    .unwrap();
    let gltf = gltf::Gltf::from_slice(&artifact.bytes).unwrap();
    let blob = gltf.blob.as_deref().unwrap();
    let primitives = gltf
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .collect::<Vec<_>>();
    assert_eq!(primitives.len(), 3);
    assert_eq!(gltf.materials().count(), 3);
    assert_eq!(gltf.images().count(), 2);
    assert_eq!(gltf.textures().count(), 2);
    assert_eq!(
        primitives
            .iter()
            .map(|primitive| primitive
                .reader(|_| Some(blob))
                .read_indices()
                .unwrap()
                .into_u32()
                .count())
            .collect::<Vec<_>>(),
        vec![3, 3, 18]
    );
    assert!(
        primitives[0]
            .reader(|_| Some(blob))
            .read_tex_coords(0)
            .is_some()
    );
    assert!(
        primitives[1]
            .reader(|_| Some(blob))
            .read_tex_coords(0)
            .is_some()
    );
    assert!(
        primitives[2]
            .reader(|_| Some(blob))
            .read_tex_coords(0)
            .is_none()
    );
    assert!(
        primitives[2]
            .material()
            .pbr_metallic_roughness()
            .base_color_texture()
            .is_none()
    );
    for image in gltf.images() {
        let gltf::image::Source::View { view, .. } = image.source() else {
            panic!("embedded image")
        };
        assert_eq!(&blob[view.offset()..view.offset() + view.length()], png);
    }

    assert!(matches!(
        export_regioned_closed_solid_triangle_mesh_glb(
            &mesh,
            &regions[..regions.len() - 1],
            EmbeddedBaseColorTextureV1 {
                media_type: EmbeddedTextureMediaTypeV1::Png,
                bytes: png,
                tex_coords: vec![[0.0, 0.0]; 6],
            },
            [0; 4],
            [0; 4],
        ),
        Err(StaticMeshExportError::TriangleRegionCountMismatch)
    ));
}

#[test]
fn texture_admission_rejects_bad_payload_uvs_and_resource_excess() {
    let mut document = sample_document();
    document.base_color_texture = Some(EmbeddedBaseColorTextureV1 {
        media_type: EmbeddedTextureMediaTypeV1::Jpeg,
        bytes: vec![0xff, 0xd8, 0xff, 0xd9],
        tex_coords: vec![[0.0, 0.0]; 3],
    });
    assert!(matches!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::TextureCoordinateCountMismatch { .. })
    ));
    document.base_color_texture.as_mut().unwrap().tex_coords = vec![[0.0, 0.0]; 4];
    document.base_color_texture.as_mut().unwrap().bytes = vec![0; 4];
    assert_eq!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::InvalidTexturePayload)
    );
    document.base_color_texture.as_mut().unwrap().bytes =
        vec![0; MAX_STATIC_MESH_TEXTURE_BYTES + 1];
    assert!(matches!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::TextureTooLarge { .. })
    ));
}

fn sample_mesh() -> ValidatedIndexedTriangleMesh {
    validate_indexed_triangle_mesh(&sample_document()).expect("sample mesh")
}

#[test]
fn glb_generation_provenance_extension_round_trips_and_legacy_is_absent() {
    let mesh = sample_mesh();
    let provenance = ori_domain::BeginnerGenerationProvenanceV1 {
        schema_version: 1,
        topology_authority_sha256: [0x17; 32],
        fold_path_certificate_sha256: Some([0x71; 32]),
        document_authority_sha256: None,
        confidence_score: 90,
        confidence_reasons: vec!["bounded_native_fold_path_v2".to_owned()],
        explicit_override: false,
        source_asset_fingerprint: "asset:glb-extension".to_owned(),
        semantic_landmark_provenance: None,
        generic_tree: None,
        reference_consensus: None,
        reference_consensus_summary: None,
    };
    let artifact = export_static_triangle_mesh_glb_with_provenance(&mesh, &provenance)
        .expect("GLB provenance export");
    assert_eq!(
        read_glb_generation_provenance(&artifact.bytes).expect("GLB provenance read"),
        Some(provenance)
    );
    let json_length = read_u32_le_at(&artifact.bytes, 12).unwrap() as usize;
    let json_end = 20 + json_length;
    let binary_length = read_u32_le_at(&artifact.bytes, json_end).unwrap() as usize;
    let mut root: serde_json::Value =
        serde_json::from_slice(&artifact.bytes[20..json_end]).unwrap();
    root["extensions"][ORIGAMI2_GENERATION_PROVENANCE_GLB_EXTENSION_V1]["provenance"]["confidence_score"] =
        serde_json::json!(89);
    let tampered = encode_glb_root_and_binary(
        &root,
        &artifact.bytes[json_end + 8..json_end + 8 + binary_length],
        MAX_STATIC_MESH_EXPORT_BYTES,
    )
    .unwrap();
    assert!(read_glb_generation_provenance(&tampered).is_err());
    let legacy =
        export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).expect("legacy GLB");
    assert_eq!(read_glb_generation_provenance(&legacy.bytes).unwrap(), None);
}

fn limits() -> StaticMeshExportLimits {
    StaticMeshExportLimits::default()
}

#[test]
fn one_admitted_mesh_exports_three_verified_deterministic_formats() {
    let mesh = sample_mesh();
    for format in [
        StaticMeshExportFormat::Obj,
        StaticMeshExportFormat::BinaryStl,
        StaticMeshExportFormat::Glb20,
    ] {
        let first = export_static_triangle_mesh(format, &mesh).expect("first export");
        let second = export_static_triangle_mesh(format, &mesh).expect("second export");
        assert_eq!(first, second);
        assert_eq!(first.vertex_count, 4);
        assert_eq!(first.triangle_count, 2);
        assert_eq!(first.media_type, format.media_type());
        assert_eq!(first.file_extension, format.file_extension());
        assert!(!first.bytes.is_empty());
    }
}

#[test]
fn schema_version_is_exact_and_unknown_fields_are_rejected_by_serde() {
    let mut document = sample_document();
    document.schema_version += 1;
    assert_eq!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::UnsupportedSchemaVersion {
            found: 2,
            latest: 1
        })
    );
    let json = serde_json::json!({
        "schema_version": 1,
        "name": "mesh",
        "positions_mm": [[0,0,0],[1,0,0],[0,1,0]],
        "normals": [[0,0,1],[0,0,1],[0,0,1]],
        "triangles": [[0,1,2]],
        "future": true
    });
    assert!(serde_json::from_value::<IndexedTriangleMeshV1>(json).is_err());
}

#[test]
fn finite_values_are_required_and_negative_zero_is_canonicalized_everywhere() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut document = sample_document();
        document.positions_mm[0][0] = value;
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::NonFinitePosition { vertex_index: 0 })
        );
        let mut document = sample_document();
        document.normals[1][2] = value;
        assert_eq!(
            validate_indexed_triangle_mesh(&document),
            Err(StaticMeshExportError::NonFiniteNormal { vertex_index: 1 })
        );
    }

    let mesh = sample_mesh();
    for vector in mesh.positions_mm.iter().chain(mesh.normals.iter()) {
        for value in vector {
            assert_ne!(value.to_bits(), (-0.0_f64).to_bits());
        }
    }
    let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
        .expect("OBJ")
        .bytes;
    assert!(!String::from_utf8(obj).expect("OBJ UTF-8").contains("-0"));
    let stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh)
        .expect("STL")
        .bytes;
    assert!(verify_binary_stl(&stl, &mesh, stl.len()));
    let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
        .expect("GLB")
        .bytes;
    assert!(verify_glb(&glb, &mesh, glb.len()));
}

#[test]
fn normals_are_counted_validated_and_canonically_normalized() {
    let mut document = sample_document();
    document.normals.pop();
    assert_eq!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::NormalCountMismatch {
            actual: 3,
            expected: 4
        })
    );
    let mut document = sample_document();
    document.normals[0] = [0.0, 0.0, 0.0];
    assert_eq!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::InvalidNormal { vertex_index: 0 })
    );
    let mesh = sample_mesh();
    assert_eq!(mesh.normals[0], [0.0, 0.0, 1.0]);
}

#[test]
fn vertex_colors_are_optional_but_must_cover_every_vertex() {
    let legacy = serde_json::json!({
        "schema_version": 1,
        "name": "mesh",
        "positions_mm": [[0,0,0],[1,0,0],[0,1,0]],
        "normals": [[0,0,1],[0,0,1],[0,0,1]],
        "triangles": [[0,1,2]]
    });
    let legacy: IndexedTriangleMeshV1 =
        serde_json::from_value(legacy).expect("legacy mesh without colors");
    assert!(legacy.vertex_colors_rgba.is_empty());

    let mut document = sample_document();
    document.vertex_colors_rgba = vec![[255, 0, 0, 255]; 3];
    assert_eq!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::VertexColorCountMismatch {
            actual: 3,
            expected: 4,
        })
    );

    document.vertex_colors_rgba.push([0, 0, 255, 255]);
    let validated = validate_indexed_triangle_mesh(&document).expect("colored mesh");
    assert_eq!(validated.vertex_colors_rgba(), document.vertex_colors_rgba);
}

#[test]
fn empty_and_unreferenced_mesh_content_is_rejected() {
    let empty = IndexedTriangleMeshV1::new("", vec![], vec![], vec![]);
    assert_eq!(
        validate_indexed_triangle_mesh(&empty),
        Err(StaticMeshExportError::NoVertices)
    );
    let mut no_triangles = sample_document();
    no_triangles.triangles.clear();
    assert_eq!(
        validate_indexed_triangle_mesh(&no_triangles),
        Err(StaticMeshExportError::NoTriangles)
    );
    let mut unreferenced = sample_document();
    unreferenced.triangles.pop();
    assert_eq!(
        validate_indexed_triangle_mesh(&unreferenced),
        Err(StaticMeshExportError::UnreferencedVertex { vertex_index: 3 })
    );
}

#[test]
fn indices_and_geometric_degeneracy_fail_closed() {
    let mut out_of_range = sample_document();
    out_of_range.triangles[0][2] = 4;
    assert_eq!(
        validate_indexed_triangle_mesh(&out_of_range),
        Err(StaticMeshExportError::IndexOutOfRange {
            triangle_index: 0,
            corner_index: 2,
            vertex_index: 4,
            vertex_count: 4
        })
    );
    let mut repeated = sample_document();
    repeated.triangles[0] = [0, 1, 1];
    assert_eq!(
        validate_indexed_triangle_mesh(&repeated),
        Err(StaticMeshExportError::RepeatedTriangleIndex { triangle_index: 0 })
    );
    let collinear = IndexedTriangleMeshV1::new(
        "line",
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![[0, 1, 2]],
    );
    assert_eq!(
        validate_indexed_triangle_mesh(&collinear),
        Err(StaticMeshExportError::DegenerateTriangle { triangle_index: 0 })
    );
}

#[test]
fn f32_overflow_and_precision_collapse_are_rejected_before_export() {
    let huge = IndexedTriangleMeshV1::new(
        "huge",
        vec![[f64::MAX, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![[0, 1, 2]],
    );
    assert_eq!(
        validate_indexed_triangle_mesh(&huge),
        Err(StaticMeshExportError::PositionNotRepresentable {
            vertex_index: 0,
            precision: StaticMeshEncodedPrecision::BinaryStlMillimetres
        })
    );

    let next = f64::from_bits(1.0_f64.to_bits() + 1);
    let collapsed = IndexedTriangleMeshV1::new(
        "collapse",
        vec![[1.0, 0.0, 0.0], [next, 0.0, 0.0], [1.0, 1.0, 0.0]],
        vec![[0.0, 0.0, 1.0]; 3],
        vec![[0, 1, 2]],
    );
    assert_eq!(
        validate_indexed_triangle_mesh(&collapsed),
        Err(StaticMeshExportError::EncodedDegenerateTriangle {
            triangle_index: 0,
            precision: StaticMeshEncodedPrecision::BinaryStlMillimetres
        })
    );
}

#[test]
fn vertex_triangle_and_name_limits_accept_exact_and_reject_one_short() {
    let document = sample_document();
    let mut exact = limits();
    exact.max_vertices = 4;
    exact.max_triangles = 2;
    exact.max_name_chars = document.name.chars().count();
    exact.max_name_bytes = document.name.len();
    validate_indexed_triangle_mesh_with_limits(&document, exact).expect("exact limits");

    let mut one_short = exact;
    one_short.max_vertices = 3;
    assert_eq!(
        validate_indexed_triangle_mesh_with_limits(&document, one_short),
        Err(StaticMeshExportError::TooManyVertices {
            actual: 4,
            maximum: 3
        })
    );
    one_short = exact;
    one_short.max_triangles = 1;
    assert_eq!(
        validate_indexed_triangle_mesh_with_limits(&document, one_short),
        Err(StaticMeshExportError::TooManyTriangles {
            actual: 2,
            maximum: 1
        })
    );
    one_short = exact;
    one_short.max_name_chars -= 1;
    assert!(matches!(
        validate_indexed_triangle_mesh_with_limits(&document, one_short),
        Err(StaticMeshExportError::NameTooManyCharacters { .. })
    ));
    one_short = exact;
    one_short.max_name_bytes -= 1;
    assert!(matches!(
        validate_indexed_triangle_mesh_with_limits(&document, one_short),
        Err(StaticMeshExportError::NameTooManyBytes { .. })
    ));
}

#[test]
fn hard_name_limits_cannot_be_relaxed_by_a_caller() {
    let mut document = sample_document();
    document.name = "a".repeat(MAX_STATIC_MESH_NAME_CHARS + 1);
    let relaxed = StaticMeshExportLimits {
        max_name_chars: usize::MAX,
        max_name_bytes: usize::MAX,
        ..limits()
    };
    assert_eq!(
        validate_indexed_triangle_mesh_with_limits(&document, relaxed),
        Err(StaticMeshExportError::NameTooManyCharacters {
            actual: MAX_STATIC_MESH_NAME_CHARS + 1,
            maximum: MAX_STATIC_MESH_NAME_CHARS
        })
    );
}

#[test]
fn output_byte_limits_accept_exact_and_reject_one_short_for_each_format() {
    let mesh = sample_mesh();
    for format in [
        StaticMeshExportFormat::Obj,
        StaticMeshExportFormat::BinaryStl,
        StaticMeshExportFormat::Glb20,
    ] {
        let artifact = export_static_triangle_mesh(format, &mesh).expect("baseline");
        let exact = StaticMeshExportLimits {
            max_output_bytes: artifact.bytes.len(),
            ..limits()
        };
        assert_eq!(
            export_static_triangle_mesh_with_limits(format, &mesh, exact)
                .expect("exact byte limit")
                .bytes,
            artifact.bytes
        );
        let one_short = StaticMeshExportLimits {
            max_output_bytes: artifact.bytes.len() - 1,
            ..limits()
        };
        assert!(matches!(
            export_static_triangle_mesh_with_limits(format, &mesh, one_short),
            Err(StaticMeshExportError::OutputTooLarge { maximum, .. })
                if maximum == artifact.bytes.len() - 1
        ));
    }
}

#[test]
fn malicious_name_is_encoded_or_json_escaped_without_record_injection() {
    let mut document = sample_document();
    document.name = "x #\u{304a}\u{308a}\"} , \"nodes\":[{\"mesh\":99}]".to_owned();
    let mesh = validate_indexed_triangle_mesh(&document).expect("safe escaped name");
    let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
        .expect("OBJ")
        .bytes;
    let obj = String::from_utf8(obj).expect("OBJ UTF-8");
    assert_eq!(obj.lines().filter(|line| line.starts_with("o ")).count(), 1);
    assert!(!obj.lines().any(|line| line.starts_with("x #")));
    assert!(obj.contains("_23"));

    let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
        .expect("GLB")
        .bytes;
    assert!(verify_glb(&glb, &mesh, glb.len()));

    document.name = "bad\nobject".to_owned();
    assert_eq!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::InvalidNameCharacter {
            character_index: 3,
            code_point: 0x0a
        })
    );
    document.name = "bad\u{2028}object".to_owned();
    assert!(matches!(
        validate_indexed_triangle_mesh(&document),
        Err(StaticMeshExportError::InvalidNameCharacter {
            code_point: 0x2028,
            ..
        })
    ));
}

#[test]
fn obj_checker_rejects_noncanonical_number_and_changed_face() {
    let mesh = sample_mesh();
    let artifact = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh).expect("OBJ");
    let mut text = String::from_utf8(artifact.bytes).expect("UTF-8");
    text = text.replacen("v 0 0 0", "v -0 0 0", 1);
    assert!(!verify_obj(text.as_bytes(), &mesh, text.len()));

    let artifact = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh).expect("OBJ");
    let mut text = String::from_utf8(artifact.bytes).expect("UTF-8");
    text = text.replacen("f 1//1 2//2 3//3", "f 1//1 3//3 2//2", 1);
    assert!(!verify_obj(text.as_bytes(), &mesh, text.len()));

    let artifact = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh).expect("OBJ");
    let mut text = String::from_utf8(artifact.bytes).expect("UTF-8");
    text = text.replacen("f 1//1 2//2 3//3", "f 1//01 2//2 3//3", 1);
    assert!(!verify_obj(text.as_bytes(), &mesh, text.len()));
}

#[test]
fn binary_stl_header_count_size_endianness_and_attributes_are_strict() {
    let mesh = sample_mesh();
    let artifact =
        export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh).expect("STL");
    assert_eq!(artifact.bytes.len(), 84 + 2 * STL_TRIANGLE_BYTES);
    assert_eq!(&artifact.bytes[..80], &stl_header(&mesh));
    assert_eq!(&artifact.bytes[80..84], &2_u32.to_le_bytes());

    let mut big_endian_count = artifact.bytes.clone();
    big_endian_count[80..84].copy_from_slice(&2_u32.to_be_bytes());
    assert!(!verify_binary_stl(
        &big_endian_count,
        &mesh,
        big_endian_count.len()
    ));
    let mut bad_attribute = artifact.bytes.clone();
    bad_attribute[132] = 1;
    assert!(!verify_binary_stl(
        &bad_attribute,
        &mesh,
        bad_attribute.len()
    ));
    let mut truncated = artifact.bytes;
    truncated.pop();
    assert!(!verify_binary_stl(&truncated, &mesh, truncated.len()));
}

#[test]
fn glb_has_aligned_chunks_fixed_property_order_and_strict_structure() {
    let mesh = sample_mesh();
    let artifact = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).expect("GLB");
    let bytes = artifact.bytes;
    assert_eq!(&bytes[..4], b"glTF");
    assert_eq!(read_u32_le_at(&bytes, 4), Some(2));
    assert_eq!(
        read_u32_le_at(&bytes, 8).and_then(|value| usize::try_from(value).ok()),
        Some(bytes.len())
    );
    let json_length = usize::try_from(read_u32_le_at(&bytes, 12).expect("JSON length"))
        .expect("usize JSON length");
    assert_eq!(json_length % 4, 0);
    let json_chunk = &bytes[20..20 + json_length];
    let json = std::str::from_utf8(json_chunk)
        .expect("JSON UTF-8")
        .trim_end_matches(' ');
    let keys = [
        "\"asset\"",
        "\"scene\"",
        "\"scenes\"",
        "\"nodes\"",
        "\"meshes\"",
        "\"materials\"",
        "\"buffers\"",
        "\"bufferViews\"",
        "\"accessors\"",
    ];
    let offsets: Vec<_> = keys
        .iter()
        .map(|key| json.find(key).expect("fixed root property"))
        .collect();
    assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(json.contains("\"POSITION\":0,\"NORMAL\":1"));
    assert!(json.contains("\"material\":0"));

    let colored =
        validate_indexed_triangle_mesh(&sample_document().with_base_color_rgba([12, 34, 56, 255]))
            .expect("colored mesh");
    let colored_glb =
        export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &colored).expect("GLB");
    let colored_json_length =
        usize::try_from(read_u32_le_at(&colored_glb.bytes, 12).unwrap()).unwrap();
    let colored_json =
        std::str::from_utf8(&colored_glb.bytes[20..20 + colored_json_length]).unwrap();
    assert!(colored_json.contains("\"baseColorFactor\":[0.047058824,0.13333334,0.21960784,1.0]"));

    let binary_header = 20 + json_length;
    assert_eq!(
        read_u32_le_at(&bytes, binary_header + 4),
        Some(GLB_BIN_CHUNK_TYPE)
    );
    let binary_length = usize::try_from(read_u32_le_at(&bytes, binary_header).expect("BIN length"))
        .expect("usize BIN");
    assert_eq!(binary_length % 4, 0);
    assert_eq!(binary_header + 8 + binary_length, bytes.len());

    let mut bad_total = bytes.clone();
    bad_total[8..12].copy_from_slice(&0_u32.to_le_bytes());
    assert!(!verify_glb(&bad_total, &mesh, bad_total.len()));
    let mut bad_offset = bytes.clone();
    let offset_token = b"\"byteOffset\":96";
    let offset_start = bad_offset
        .windows(offset_token.len())
        .position(|window| window == offset_token)
        .expect("index buffer offset");
    bad_offset[offset_start + offset_token.len() - 1] = b'2';
    assert!(!verify_glb(&bad_offset, &mesh, bad_offset.len()));

    let mut bad_index = bytes.clone();
    let binary_start = binary_header + GLB_CHUNK_HEADER_BYTES;
    let index_start = binary_start + 4 * 3 * 2 * mesh.positions_mm.len();
    bad_index[index_start..index_start + 4].copy_from_slice(&3_u32.to_le_bytes());
    assert!(!verify_glb(&bad_index, &mesh, bad_index.len()));

    let mut bad_chunk_type = bytes;
    bad_chunk_type[16..20].copy_from_slice(&0_u32.to_le_bytes());
    assert!(!verify_glb(&bad_chunk_type, &mesh, bad_chunk_type.len()));
}

#[test]
fn glb_preserves_normalized_rgba_vertex_colors() {
    let colors = vec![
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 128],
    ];
    let mesh =
        validate_indexed_triangle_mesh(&sample_document().with_vertex_colors_rgba(colors.clone()))
            .expect("colored mesh");
    let artifact = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh).expect("GLB");
    let json_length = usize::try_from(read_u32_le_at(&artifact.bytes, 12).unwrap()).unwrap();
    let json = std::str::from_utf8(&artifact.bytes[20..20 + json_length]).unwrap();
    assert!(json.contains("\"COLOR_0\":3"));
    assert!(
        json.contains("\"componentType\":5121,\"count\":4,\"type\":\"VEC4\",\"normalized\":true")
    );
    assert!(verify_glb(&artifact.bytes, &mesh, artifact.bytes.len()));

    let binary_header = 20 + json_length;
    let binary_start = binary_header + GLB_CHUNK_HEADER_BYTES;
    let color_start = binary_start + mesh.positions_mm.len() * 24;
    assert_eq!(
        &artifact.bytes[color_start..color_start + colors.len() * 4],
        colors.as_flattened()
    );
    let mut changed = artifact.bytes;
    changed[color_start] = 0;
    assert!(!verify_glb(&changed, &mesh, changed.len()));
}

#[test]
fn ecosystem_readers_accept_all_three_interchange_formats() {
    use std::io::Cursor;

    let colors = vec![
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 128],
    ];
    let mesh =
        validate_indexed_triangle_mesh(&sample_document().with_vertex_colors_rgba(colors.clone()))
            .expect("colored mesh");

    let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
        .expect("OBJ")
        .bytes;
    let (models, materials) = tobj::load_obj_buf(
        &mut Cursor::new(obj),
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..tobj::LoadOptions::default()
        },
        |_| Ok((Vec::new(), Default::default())),
    )
    .expect("tobj accepts OBJ");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].mesh.positions.len(), mesh.positions_mm.len() * 3);
    assert_eq!(models[0].mesh.indices.len(), mesh.triangles.len() * 3);
    assert!(materials.expect("material result").is_empty());

    let stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh)
        .expect("STL")
        .bytes;
    let parsed_stl = stl_io::read_stl(&mut Cursor::new(stl)).expect("stl_io accepts STL");
    assert_eq!(parsed_stl.faces.len(), mesh.triangles.len());

    let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
        .expect("GLB")
        .bytes;
    let parsed_glb = gltf::Gltf::from_slice(&glb).expect("gltf validator accepts GLB");
    assert_eq!(parsed_glb.scenes().count(), 1);
    let primitive = parsed_glb
        .meshes()
        .next()
        .and_then(|mesh| mesh.primitives().next())
        .expect("one primitive");
    let reader = primitive.reader(|_| parsed_glb.blob.as_deref());
    assert_eq!(
        reader.read_positions().expect("positions").count(),
        mesh.positions_mm.len()
    );
    assert_eq!(
        reader.read_indices().expect("indices").into_u32().count(),
        mesh.triangles.len() * 3
    );
    let parsed_colors: Vec<_> = reader
        .read_colors(0)
        .expect("vertex colors")
        .into_rgba_u8()
        .collect();
    assert_eq!(parsed_colors, colors);
}

#[test]
fn glb_node_rotation_maps_source_right_forward_up_without_reflection() {
    let transform = |vector: [f32; 3]| {
        [
            GLTF_NODE_MATRIX[0] * vector[0]
                + GLTF_NODE_MATRIX[4] * vector[1]
                + GLTF_NODE_MATRIX[8] * vector[2],
            GLTF_NODE_MATRIX[1] * vector[0]
                + GLTF_NODE_MATRIX[5] * vector[1]
                + GLTF_NODE_MATRIX[9] * vector[2],
            GLTF_NODE_MATRIX[2] * vector[0]
                + GLTF_NODE_MATRIX[6] * vector[1]
                + GLTF_NODE_MATRIX[10] * vector[2],
        ]
    };
    assert_eq!(transform([1.0, 0.0, 0.0]), [-1.0, 0.0, 0.0]);
    assert_eq!(transform([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
    assert_eq!(transform([0.0, 0.0, 1.0]), [0.0, 1.0, 0.0]);

    let first = transform([1.0, 0.0, 0.0]);
    let second = transform([0.0, 1.0, 0.0]);
    let third = transform([0.0, 0.0, 1.0]);
    let determinant = first[0] * (second[1] * third[2] - second[2] * third[1])
        - second[0] * (first[1] * third[2] - first[2] * third[1])
        + third[0] * (first[1] * second[2] - first[2] * second[1]);
    assert_eq!(determinant, 1.0);
}

#[test]
fn all_formats_preserve_one_geometry_with_only_documented_unit_conversion() {
    let mesh = sample_mesh();
    let obj = export_static_triangle_mesh(StaticMeshExportFormat::Obj, &mesh)
        .expect("OBJ")
        .bytes;
    let obj_text = std::str::from_utf8(&obj).expect("OBJ UTF-8");
    let obj_positions: Vec<[f64; 3]> = obj_text
        .lines()
        .filter_map(|line| line.strip_prefix("v "))
        .map(|line| {
            let values: Vec<f64> = line
                .split(' ')
                .map(|value| value.parse().expect("OBJ number"))
                .collect();
            [values[0], values[1], values[2]]
        })
        .collect();
    assert_eq!(obj_positions, mesh.positions_mm);

    let glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &mesh)
        .expect("GLB")
        .bytes;
    let json_length =
        usize::try_from(read_u32_le_at(&glb, 12).expect("JSON length")).expect("usize");
    let binary_start = 20 + json_length + 8;
    let mut cursor = binary_start;
    for source in &mesh.positions_mm {
        for component in source {
            let encoded = read_f32_le(&glb, &mut cursor).expect("GLB position");
            assert_eq!(
                encoded.to_bits(),
                canonical_zero_f32((*component * 0.001) as f32).to_bits()
            );
        }
    }

    let stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &mesh)
        .expect("STL")
        .bytes;
    let mut cursor = 84;
    for triangle in &mesh.triangles {
        cursor += 12;
        for index in triangle {
            for component in mesh.positions_mm[usize::try_from(*index).expect("usize index")] {
                let encoded = read_f32_le(&stl, &mut cursor).expect("STL position");
                assert_eq!(encoded.to_bits(), (component as f32).to_bits());
            }
        }
        cursor += 2;
    }
    assert_eq!(cursor, stl.len());
}

#[test]
fn input_order_is_preserved_and_winding_changes_output_deterministically() {
    let first = sample_mesh();
    let mut reversed_document = sample_document();
    for triangle in &mut reversed_document.triangles {
        triangle.swap(1, 2);
    }
    let reversed = validate_indexed_triangle_mesh(&reversed_document).expect("reversed winding");
    for format in [
        StaticMeshExportFormat::Obj,
        StaticMeshExportFormat::BinaryStl,
        StaticMeshExportFormat::Glb20,
    ] {
        let first_bytes = export_static_triangle_mesh(format, &first)
            .expect("first")
            .bytes;
        let reversed_bytes = export_static_triangle_mesh(format, &reversed)
            .expect("reversed")
            .bytes;
        assert_ne!(first_bytes, reversed_bytes);
    }
}

#[test]
fn public_format_metadata_is_fixed() {
    assert_eq!(StaticMeshExportFormat::Obj.media_type(), "model/obj");
    assert_eq!(StaticMeshExportFormat::Obj.file_extension(), "obj");
    assert_eq!(StaticMeshExportFormat::BinaryStl.media_type(), "model/stl");
    assert_eq!(StaticMeshExportFormat::BinaryStl.file_extension(), "stl");
    assert_eq!(
        StaticMeshExportFormat::Glb20.media_type(),
        "model/gltf-binary"
    );
    assert_eq!(StaticMeshExportFormat::Glb20.file_extension(), "glb");
}
