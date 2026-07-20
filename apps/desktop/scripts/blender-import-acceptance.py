import json
import math
import pathlib
import sys

import bpy


artifacts = pathlib.Path(sys.argv[sys.argv.index("--") + 1]).resolve()
CASES = {
    "static.obj": {"bounds": ([0, 0, 0], [10, 10, 0]), "materials": 0, "images": 0, "animation": False},
    "static.stl": {"bounds": ([0, 0, 0], [10, 10, 0]), "materials": 0, "images": 0, "animation": False},
    "static.glb": {"bounds": ([-0.01, -0.01, 0], [0, 0, 0]), "materials": 1, "images": 0, "animation": False},
    "textured.glb": {"bounds": ([-0.01, -0.01, 0], [0, 0, 0]), "materials": 1, "images": 1, "animation": False},
    "animated.glb": {"bounds": ([-0.01, -0.01, 0], [0, 0, 0]), "materials": 1, "images": 0, "animation": True},
}


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def close(actual, expected, tolerance=1e-6):
    return math.isclose(actual, expected, rel_tol=0.0, abs_tol=tolerance)


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def import_artifact(path):
    suffix = path.suffix
    if suffix == ".obj":
        result = bpy.ops.wm.obj_import(filepath=str(path), forward_axis="Y", up_axis="Z")
    elif suffix == ".stl":
        result = bpy.ops.wm.stl_import(filepath=str(path), forward_axis="Y", up_axis="Z")
    else:
        result = bpy.ops.import_scene.gltf(filepath=str(path))
    require(result == {"FINISHED"}, f"{path.name}: importer returned {result}")


def world_bounds(mesh_object):
    evaluated = mesh_object.evaluated_get(bpy.context.evaluated_depsgraph_get())
    points = [evaluated.matrix_world @ vertex.co for vertex in evaluated.data.vertices]
    return (
        [min(point[axis] for point in points) for axis in range(3)],
        [max(point[axis] for point in points) for axis in range(3)],
    )


def is_closed_manifold(mesh):
    edge_faces = [0] * len(mesh.edges)
    for polygon in mesh.polygons:
        for edge_index in polygon.edge_keys:
            key = tuple(sorted(edge_index))
            for edge in mesh.edges:
                if tuple(sorted(edge.vertices)) == key:
                    edge_faces[edge.index] += 1
                    break
    return bool(edge_faces) and all(count == 2 for count in edge_faces)


reports = []
for filename, expected in CASES.items():
    reset()
    path = artifacts / filename
    require(path.is_file(), f"missing artifact: {path}")
    import_artifact(path)
    meshes = [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
    require(len(meshes) == 1, f"{filename}: expected one mesh, got {len(meshes)}")
    obj = meshes[0]
    require(len(obj.data.vertices) == 4, f"{filename}: vertex count")
    require(len(obj.data.polygons) == 2, f"{filename}: triangle count")
    minimum, maximum = world_bounds(obj)
    expected_minimum, expected_maximum = expected["bounds"]
    require(all(close(value, expected_minimum[axis]) for axis, value in enumerate(minimum)),
            f"{filename}: minimum {minimum}")
    require(all(close(value, expected_maximum[axis]) for axis, value in enumerate(maximum)),
            f"{filename}: maximum {maximum}")
    require(len(bpy.data.materials) == expected["materials"], f"{filename}: material count")
    require(len(bpy.data.images) == expected["images"], f"{filename}: image count")
    closed_manifold = is_closed_manifold(obj.data)
    require(not closed_manifold, f"{filename}: fixture must remain an open sheet")

    actions = len(bpy.data.actions)
    if expected["animation"]:
        require(actions == 1, f"{filename}: expected one animation action, got {actions}")
        require(obj.data.shape_keys is not None, f"{filename}: missing morph targets")
        bpy.context.scene.frame_set(1)
        start = sum(world_bounds(obj), [])
        bpy.context.scene.frame_set(13)
        middle = sum(world_bounds(obj), [])
        require(any(not close(left, right) for left, right in zip(start, middle)),
                f"{filename}: animation did not change bounds")
    else:
        require(actions == 0, f"{filename}: unexpected animation actions")

    reports.append({
        "artifact": filename,
        "axis": "Blender right-handed Z-up",
        "unit": "millimetre" if path.suffix in {".obj", ".stl"} else "metre",
        "bounds": [minimum, maximum],
        "closed_manifold": closed_manifold,
        "images": len(bpy.data.images),
        "materials": len(bpy.data.materials),
        "animations": actions,
    })

print("ORIGAMI2_BLENDER_ACCEPTANCE=" + json.dumps(reports, sort_keys=True))
