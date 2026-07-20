use std::{env, fs, path::PathBuf};

use ori_formats::{
    EmbeddedBaseColorTextureV1, EmbeddedTextureMediaTypeV1, IndexedTriangleMeshAnimationV1,
    IndexedTriangleMeshV1, StaticMeshExportFormat, export_animated_triangle_mesh_glb,
    export_static_triangle_mesh, validate_indexed_triangle_mesh,
};

const PNG_1X1_RGBA: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31, 0, 5,
    0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn mesh(name: &str, z: f64) -> IndexedTriangleMeshV1 {
    IndexedTriangleMeshV1::new(
        name,
        vec![
            [0.0, 0.0, z],
            [10.0, 0.0, z],
            [10.0, 10.0, z],
            [0.0, 10.0, z],
        ],
        vec![[0.0, 0.0, 1.0]; 4],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

fn animated_mesh(deformation: f64) -> IndexedTriangleMeshV1 {
    IndexedTriangleMeshV1::new(
        "animated",
        vec![
            [0.0, 0.0, 0.0],
            [10.0 + deformation, 0.0, 0.0],
            [10.0 + deformation, 10.0, deformation],
            [0.0, 10.0, 0.0],
        ],
        vec![[0.0, 0.0, 1.0]; 4],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

fn positive_thickness_mesh() -> IndexedTriangleMeshV1 {
    IndexedTriangleMeshV1::new(
        "positive-thickness",
        vec![
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
            [0.0, 0.0, 2.0],
            [10.0, 0.0, 2.0],
            [10.0, 10.0, 2.0],
            [0.0, 10.0, 2.0],
        ],
        vec![[0.0, 0.0, 1.0]; 8],
        vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ],
    )
}

fn main() {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: generate_gltf_validator_fixtures OUTPUT_DIRECTORY");
    fs::create_dir_all(&output).expect("create output directory");

    let static_mesh = validate_indexed_triangle_mesh(&mesh("static", 0.0)).expect("static mesh");
    let static_glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &static_mesh)
        .expect("static GLB");
    fs::write(output.join("static.glb"), static_glb.bytes).expect("write static GLB");
    let static_obj =
        export_static_triangle_mesh(StaticMeshExportFormat::Obj, &static_mesh).expect("static OBJ");
    fs::write(output.join("static.obj"), static_obj.bytes).expect("write static OBJ");
    let static_stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &static_mesh)
        .expect("static STL");
    fs::write(output.join("static.stl"), static_stl.bytes).expect("write static STL");
    let solid =
        validate_indexed_triangle_mesh(&positive_thickness_mesh()).expect("positive thickness");
    let solid_stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &solid)
        .expect("positive-thickness STL");
    fs::write(output.join("positive-thickness.stl"), solid_stl.bytes)
        .expect("write positive-thickness STL");
    let mut unproven = positive_thickness_mesh();
    unproven.name = "unproven-nonmanifold".into();
    unproven.triangles.push([4, 5, 6]);
    let unproven = validate_indexed_triangle_mesh(&unproven).expect("unproven non-manifold mesh");
    let unproven_stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &unproven)
        .expect("unproven non-manifold STL");
    fs::write(output.join("unproven-nonmanifold.stl"), unproven_stl.bytes)
        .expect("write unproven non-manifold STL");

    let textured = mesh("textured", 0.0).with_base_color_texture(EmbeddedBaseColorTextureV1 {
        media_type: EmbeddedTextureMediaTypeV1::Png,
        bytes: PNG_1X1_RGBA.to_vec(),
        tex_coords: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    });
    let textured = validate_indexed_triangle_mesh(&textured).expect("textured mesh");
    let textured_glb = export_static_triangle_mesh(StaticMeshExportFormat::Glb20, &textured)
        .expect("textured GLB");
    fs::write(output.join("textured.glb"), textured_glb.bytes).expect("write textured GLB");

    let animation = IndexedTriangleMeshAnimationV1::new(
        vec![0.0, 0.5, 1.0],
        vec![animated_mesh(0.0), animated_mesh(2.0), animated_mesh(5.0)],
    );
    let animated_glb = export_animated_triangle_mesh_glb(&animation).expect("animated GLB");
    fs::write(output.join("animated.glb"), animated_glb.bytes).expect("write animated GLB");
}
