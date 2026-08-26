#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import re
import struct
from dataclasses import dataclass
from pathlib import Path

SLOT_BY_COMPONENT = {
    "head": "head",
    "hair": "hair",
    "uppr": "upper",
    "lowr": "lower",
    "hand": "hands",
    "accs": "accessories",
    "decl": "decals",
}

@dataclass
class Geometry:
    name: str
    slot: str
    positions: list[tuple[float, float, float]]
    normals: list[tuple[float, float, float]]
    uvs: list[tuple[float, float]]
    indices: list[int]


def _vec3(text: str) -> tuple[float, float, float]:
    values = [float(v) for v in text.split()]
    if len(values) != 3:
        raise ValueError(f"expected vec3, got {text!r}")
    return values[0], values[1], values[2]


def _first(regex: str, text: str, label: str, flags: int = 0) -> re.Match[str]:
    match = re.search(regex, text, flags)
    if not match:
        raise ValueError(f"missing {label}")
    return match


def _component_slot(name: str) -> str:
    prefix = name.split("_", 1)[0].lower()
    return SLOT_BY_COMPONENT.get(prefix, prefix)


def parse_mesh(mesh_path: Path, odr_min: tuple[float, float, float], odr_max: tuple[float, float, float], component: str) -> list[Geometry]:
    text = mesh_path.read_text(encoding="utf-8", errors="replace")
    local_min = _vec3(_first(r"\bMin\s+([^\r\n]+)", text, "mesh Min").group(1))
    local_max = _vec3(_first(r"\bMax\s+([^\r\n]+)", text, "mesh Max").group(1))
    offset = tuple(((odr_min[i] + odr_max[i]) - (local_min[i] + local_max[i])) * 0.5 for i in range(3))
    pattern = re.compile(
        r"Geometry\s*\{\s*"
        r"ShaderIndex\s+(\d+).*?"
        r"Indices\s+(\d+)\s*\{(.*?)\}\s*"
        r"Vertices\s+(\d+)\s*\{(.*?)\}\s*\}",
        re.S,
    )
    out: list[Geometry] = []
    slot = _component_slot(component)
    for geometry_index, match in enumerate(pattern.finditer(text)):
        expected_indices = int(match.group(2))
        indices = [int(value) for value in re.findall(r"\d+", match.group(3))]
        if len(indices) != expected_indices:
            raise ValueError(f"{mesh_path}: index count {len(indices)} != {expected_indices}")
        expected_vertices = int(match.group(4))
        positions: list[tuple[float, float, float]] = []
        normals: list[tuple[float, float, float]] = []
        uvs: list[tuple[float, float]] = []
        for line in match.group(5).splitlines():
            line = line.strip()
            if not line or "/" not in line:
                continue
            groups = [group.strip() for group in line.split("/")]
            if len(groups) < 7:
                raise ValueError(f"{mesh_path}: unsupported vertex record {line[:120]!r}")
            p = _vec3(groups[0])
            n = _vec3(groups[3])
            uv_values = [float(v) for v in groups[6].split()]
            if len(uv_values) < 2:
                raise ValueError(f"{mesh_path}: malformed uv0 record")
            positions.append((p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]))
            normals.append(n)
            uvs.append((uv_values[0], uv_values[1]))
        if len(positions) != expected_vertices:
            raise ValueError(f"{mesh_path}: vertex count {len(positions)} != {expected_vertices}")
        if any(index < 0 or index >= expected_vertices for index in indices):
            raise ValueError(f"{mesh_path}: index outside vertex range")
        out.append(Geometry(
            name=f"{slot}_{geometry_index}" if geometry_index else slot,
            slot=slot,
            positions=positions,
            normals=normals,
            uvs=uvs,
            indices=indices,
        ))
    if not out:
        raise ValueError(f"{mesh_path}: no Geometry blocks decoded")
    return out


def parse_odd(odd_path: Path) -> tuple[list[Geometry], float, float]:
    odd_text = odd_path.read_text(encoding="utf-8", errors="replace")
    refs = [line.strip() for line in odd_text.splitlines() if line.strip().lower().endswith(".odr")]
    if not refs:
        raise ValueError(f"{odd_path}: no ODR references")
    geometries: list[Geometry] = []
    global_min_z = math.inf
    global_max_z = -math.inf
    for ref in refs:
        odr_path = odd_path.parent / Path(ref.replace("\\", "/"))
        odr_text = odr_path.read_text(encoding="utf-8", errors="replace")
        odr_min = _vec3(_first(r"AABBMin\s+([^\r\n]+)", odr_text, "AABBMin", re.I).group(1))
        odr_max = _vec3(_first(r"AABBMax\s+([^\r\n]+)", odr_text, "AABBMax", re.I).group(1))
        mesh_ref = _first(r"High\s+[^\r\n]+\s*\{\s*([^\s]+\.mesh)", odr_text, "High mesh", re.S).group(1)
        mesh_path = odr_path.parent / Path(mesh_ref.replace("\\", "/"))
        component = odr_path.stem
        geometries.extend(parse_mesh(mesh_path, odr_min, odr_max, component))
        global_min_z = min(global_min_z, odr_min[2])
        global_max_z = max(global_max_z, odr_max[2])
    return geometries, global_min_z, global_max_z


def _align4(blob: bytearray) -> None:
    while len(blob) & 3:
        blob.append(0)


def build_glb(geometries: list[Geometry], min_z: float, max_z: float, target_height: float, output: Path) -> dict[str, object]:
    if not math.isfinite(min_z) or not math.isfinite(max_z) or max_z <= min_z:
        raise ValueError("invalid authored vertical bounds")
    scale = target_height / (max_z - min_z)
    slots = list(dict.fromkeys(geometry.slot for geometry in geometries))
    material_index = {slot: i for i, slot in enumerate(slots)}
    binary = bytearray()
    views: list[dict[str, object]] = []
    accessors: list[dict[str, object]] = []
    meshes: list[dict[str, object]] = []
    nodes: list[dict[str, object]] = []

    def append_view(data: bytes, target: int | None = None) -> int:
        _align4(binary)
        offset = len(binary)
        binary.extend(data)
        view: dict[str, object] = {"buffer": 0, "byteOffset": offset, "byteLength": len(data)}
        if target is not None:
            view["target"] = target
        views.append(view)
        return len(views) - 1

    def append_accessor(view: int, component_type: int, count: int, kind: str, minimum=None, maximum=None) -> int:
        item: dict[str, object] = {"bufferView": view, "componentType": component_type, "count": count, "type": kind}
        if minimum is not None:
            item["min"] = minimum
        if maximum is not None:
            item["max"] = maximum
        accessors.append(item)
        return len(accessors) - 1

    total_vertices = 0
    total_indices = 0
    for geometry in geometries:
        transformed_positions = [(p[0] * scale, (p[2] - min_z) * scale, -p[1] * scale) for p in geometry.positions]
        transformed_normals = []
        for n in geometry.normals:
            x, y, z = n[0], n[2], -n[1]
            length = math.sqrt(x*x + y*y + z*z) or 1.0
            transformed_normals.append((x/length, y/length, z/length))
        pos_min = [min(p[i] for p in transformed_positions) for i in range(3)]
        pos_max = [max(p[i] for p in transformed_positions) for i in range(3)]
        pos_bytes = b"".join(struct.pack("<3f", *p) for p in transformed_positions)
        nrm_bytes = b"".join(struct.pack("<3f", *n) for n in transformed_normals)
        uv_bytes = b"".join(struct.pack("<2f", *uv) for uv in geometry.uvs)
        idx_bytes = b"".join(struct.pack("<I", index) for index in geometry.indices)
        pos_acc = append_accessor(append_view(pos_bytes, 34962), 5126, len(transformed_positions), "VEC3", pos_min, pos_max)
        nrm_acc = append_accessor(append_view(nrm_bytes, 34962), 5126, len(transformed_normals), "VEC3")
        uv_acc = append_accessor(append_view(uv_bytes, 34962), 5126, len(geometry.uvs), "VEC2")
        idx_acc = append_accessor(append_view(idx_bytes, 34963), 5125, len(geometry.indices), "SCALAR")
        meshes.append({
            "name": geometry.name,
            "primitives": [{
                "attributes": {"POSITION": pos_acc, "NORMAL": nrm_acc, "TEXCOORD_0": uv_acc},
                "indices": idx_acc,
                "material": material_index[geometry.slot],
                "mode": 4,
            }],
            "extras": {"newengine_material_slot": geometry.slot},
        })
        nodes.append({"name": geometry.name, "mesh": len(meshes) - 1})
        total_vertices += len(transformed_positions)
        total_indices += len(geometry.indices)

    materials = []
    for slot in slots:
        materials.append({
            "name": slot,
            "doubleSided": slot in {"hair", "decals"},
            "pbrMetallicRoughness": {"baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 0.7},
        })
    document = {
        "asset": {"version": "2.0", "generator": "NorthStar import_openformats_ped_glb.py"},
        "scene": 0,
        "scenes": [{"name": "csb_abigail", "nodes": list(range(len(nodes)))}],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "bufferViews": views,
        "accessors": accessors,
        "buffers": [{"byteLength": len(binary)}],
        "extras": {
            "source_format": "rage.openformats.odd.odr.mesh",
            "target_height": target_height,
            "authored_vertical_bounds": [min_z, max_z],
            "scale": scale,
        },
    }
    json_bytes = json.dumps(document, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    while len(json_bytes) & 3:
        json_bytes += b" "
    _align4(binary)
    total_length = 12 + 8 + len(json_bytes) + 8 + len(binary)
    glb = bytearray(struct.pack("<4sII", b"glTF", 2, total_length))
    glb += struct.pack("<I4s", len(json_bytes), b"JSON") + json_bytes
    glb += struct.pack("<I4s", len(binary), b"BIN\x00") + binary
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(glb)
    return {
        "geometries": len(geometries),
        "materials": slots,
        "vertices": total_vertices,
        "indices": total_indices,
        "triangles": total_indices // 3,
        "target_height": target_height,
        "scale": scale,
        "bytes": len(glb),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Import a RAGE/OpenFormats ped ODD/ODR/MESH aggregate into a NewEngine-ready GLB bind pose.")
    parser.add_argument("--odd", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--target-height", type=float, default=1.78)
    args = parser.parse_args()
    odd = Path(args.odd).resolve()
    output = Path(args.output).resolve()
    geometries, min_z, max_z = parse_odd(odd)
    summary = build_glb(geometries, min_z, max_z, args.target_height, output)
    print(json.dumps(summary, indent=2))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
