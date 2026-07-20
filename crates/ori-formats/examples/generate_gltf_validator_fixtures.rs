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
        vec![
            mesh("animated", 0.0),
            mesh("animated", 2.0),
            mesh("animated", 5.0),
        ],
    );
    let animated_glb = export_animated_triangle_mesh_glb(&animation).expect("animated GLB");
    fs::write(output.join("animated.glb"), animated_glb.bytes).expect("write animated GLB");
}
