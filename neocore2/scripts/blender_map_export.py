#!/usr/bin/env python3
"""Blender-side exporter for the North Star discrete map importer.

Executed by Blender in background mode. It exports one local-space OBJ per
unique mesh datablock plus a JSON manifest of scene placements. Repeated Blender
objects therefore become placements, not duplicated geometry assets.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

import bpy
from mathutils import Matrix


def parse_args() -> argparse.Namespace:
    argv = sys.argv
    argv = argv[argv.index("--") + 1 :] if "--" in argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--geometry-dir", required=True, type=Path)
    return parser.parse_args(argv)


def slug(value: str) -> str:
    value = re.sub(r"[^a-zA-Z0-9_]+", "_", value.strip()).strip("_").lower()
    return value or "mesh"


def unique_slug(value: str, used: set[str]) -> str:
    base = slug(value)
    candidate = base
    index = 2
    while candidate in used:
        candidate = f"{base}_{index}"
        index += 1
    used.add(candidate)
    return candidate


def bool_prop(obj: bpy.types.Object, name: str, default: bool = False) -> bool:
    value = obj.get(name, default)
    if isinstance(value, str):
        return value.strip().lower() in {"1", "true", "yes", "on"}
    return bool(value)


def string_prop(obj: bpy.types.Object, name: str, default: str = "") -> str:
    value = obj.get(name, default)
    return str(value).strip() if value is not None else default


def engine_matrix(blender_matrix: Matrix) -> Matrix:
    # Blender Z-up -> North Star/glTF-style Y-up. The transform has determinant
    # +1, so triangle winding is preserved.
    conversion = Matrix(
        (
            (1.0, 0.0, 0.0, 0.0),
            (0.0, 0.0, 1.0, 0.0),
            (0.0, -1.0, 0.0, 0.0),
            (0.0, 0.0, 0.0, 1.0),
        )
    )
    return conversion @ blender_matrix @ conversion.inverted()


def engine_point(co) -> tuple[float, float, float]:
    return float(co.x), float(co.z), float(-co.y)


def transform_payload(obj: bpy.types.Object) -> dict[str, list[float]]:
    matrix = engine_matrix(obj.matrix_world.copy())
    location, rotation, scale = matrix.decompose()
    # Yaw around Y, pitch around X, roll around Z.
    euler = rotation.to_euler("YXZ")
    return {
        "position": [float(location.x), float(location.y), float(location.z)],
        "rotation_ypr": [float(euler.y), float(euler.x), float(euler.z)],
        "scale": [float(scale.x), float(scale.y), float(scale.z)],
    }


def write_mesh_obj(path: Path, asset_id: str, source: bpy.types.Object) -> None:
    """Write evaluated mesh geometry in engine local coordinates.

    OBJ is deliberate here: the existing YDD packer accepts repeated OBJ inputs,
    and one OBJ file maps cleanly to one resident `file.ydd@entry` identity.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    depsgraph = bpy.context.evaluated_depsgraph_get()
    evaluated = source.evaluated_get(depsgraph)
    mesh = evaluated.to_mesh()
    if mesh is None:
        raise RuntimeError(f"could not evaluate mesh object {source.name!r}")
    try:
        mesh.calc_loop_triangles()
        uv_data = mesh.uv_layers.active.data if mesh.uv_layers.active is not None else None
        lines: list[str] = [f"# North Star Blender map mesh: {source.name}", f"o {asset_id}"]
        for vertex in mesh.vertices:
            x, y, z = engine_point(vertex.co)
            lines.append(f"v {x:.9g} {y:.9g} {z:.9g}")

        uv_index = 1
        faces: list[str] = []
        uv_lines: list[str] = []
        for triangle in mesh.loop_triangles:
            tokens: list[str] = []
            for vertex_index, loop_index in zip(triangle.vertices, triangle.loops):
                if uv_data is not None:
                    uv = uv_data[loop_index].uv
                    uv_lines.append(f"vt {float(uv.x):.9g} {float(uv.y):.9g}")
                    tokens.append(f"{int(vertex_index) + 1}/{uv_index}")
                    uv_index += 1
                else:
                    tokens.append(str(int(vertex_index) + 1))
            faces.append("f " + " ".join(tokens))
        lines.extend(uv_lines)
        lines.extend(faces)
        path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    finally:
        evaluated.to_mesh_clear()


def export_unique_geometry(
    geometry_dir: Path,
    mesh_assets: list[tuple[str, bpy.types.Object]],
) -> dict[str, str]:
    geometry_dir.mkdir(parents=True, exist_ok=True)
    outputs: dict[str, str] = {}
    for asset_id, source in mesh_assets:
        path = geometry_dir / f"{asset_id}.obj"
        write_mesh_obj(path, asset_id, source)
        outputs[asset_id] = str(path)
    return outputs


def main() -> int:
    options = parse_args()
    options.manifest.parent.mkdir(parents=True, exist_ok=True)

    scene_objects = [
        obj
        for obj in bpy.context.scene.objects
        if obj.type == "MESH"
        and not obj.hide_render
        and not bool_prop(obj, "ns_map_ignore", False)
    ]
    scene_objects.sort(key=lambda obj: obj.name.lower())

    used_asset_ids: set[str] = set()
    used_instance_ids: set[str] = set()
    mesh_key_to_asset: dict[int, str] = {}
    mesh_assets: list[tuple[str, bpy.types.Object]] = []
    assets: dict[str, dict] = {}
    instances: list[dict] = []

    for obj in scene_objects:
        definition_override = string_prop(obj, "ns_definition")
        asset_id = ""
        if not definition_override:
            # Objects sharing one Blender mesh datablock reuse one runtime YDD.
            # A modifier-bearing object may opt out by setting ns_unique_mesh=true.
            key = int(obj.data.as_pointer())
            if bool_prop(obj, "ns_unique_mesh", False):
                key = int(obj.as_pointer())
            asset_id = mesh_key_to_asset.get(key, "")
            if not asset_id:
                asset_id = unique_slug(obj.data.name or obj.name, used_asset_ids)
                mesh_key_to_asset[key] = asset_id
                mesh_assets.append((asset_id, obj))
                material_ref = string_prop(obj, "ns_material")
                if not material_ref and obj.active_material is not None:
                    material_ref = string_prop(obj.active_material, "ns_material")
                assets[asset_id] = {
                    "asset_id": asset_id,
                    "source_object": obj.name,
                    "material_ref": material_ref,
                    "collision": bool_prop(obj, "ns_collision", False),
                }

        instance_id = unique_slug(obj.name, used_instance_ids)
        transform = transform_payload(obj)
        tags = [
            item.strip().lower()
            for item in string_prop(obj, "ns_tags").split(",")
            if item.strip()
        ]
        cell_override = None
        if "ns_cell_x" in obj and "ns_cell_z" in obj:
            cell_override = [int(obj["ns_cell_x"]), int(obj["ns_cell_z"])]

        instances.append(
            {
                "id": instance_id,
                "object_name": obj.name,
                "asset_id": asset_id,
                "definition_ref": definition_override,
                "apply_mode": string_prop(obj, "ns_apply_mode", "instantiate") or "instantiate",
                "enabled": not bool_prop(obj, "ns_disabled", False),
                "tags": sorted(set(tags)),
                "cell_override": cell_override,
                **transform,
            }
        )

    geometry_files = export_unique_geometry(options.geometry_dir, mesh_assets)
    for asset_id, geometry_file in geometry_files.items():
        assets[asset_id]["geometry_file"] = geometry_file

    payload = {
        "schema": "northstar.blender_map_export.v1",
        "blender_version": bpy.app.version_string,
        "scene": bpy.context.scene.name,
        "source_file": bpy.data.filepath,
        "geometry_dir": str(options.geometry_dir),
        "assets": [assets[key] for key in sorted(assets)],
        "instances": instances,
        "counts": {
            "mesh_objects": len(scene_objects),
            "unique_mesh_assets": len(mesh_assets),
            "instances": len(instances),
        },
        "authoring_convention": {
            "ns_definition": "Optional existing .ytyp@entry ref; skips geometry import for this object.",
            "ns_material": "Optional .nemat@entry material ref on object or active material.",
            "ns_collision": "Optional static collision intent metadata.",
            "ns_map_ignore": "Exclude object from map import.",
            "ns_unique_mesh": "Force a separate runtime mesh even when Blender mesh datablock is shared.",
            "ns_cell_x/ns_cell_z": "Optional manual cell override.",
        },
    }
    options.manifest.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(
        "NORTHSTAR_BLENDER_MAP_EXPORT_OK "
        f"objects={len(scene_objects)} assets={len(mesh_assets)} instances={len(instances)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
