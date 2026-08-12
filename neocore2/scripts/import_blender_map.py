#!/usr/bin/env python3
"""Import/replace a North Star map directly from a Blender .blend scene.

Pipeline:
  .blend -> Blender headless export -> one OBJ per unique mesh + placement manifest
        -> one reusable YDD per unique mesh
        -> generated YTYP definition entries
        -> discrete YMAP v2 cells
        -> native asset build plan update

The runtime never parses .blend. Blender is an authoring/import dependency only.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
import zlib
from pathlib import Path
from typing import Any

PLAN_SCHEMA = "northstar.native_asset_build_plan.v1"
IMPORT_SCHEMA = "northstar.blender_map_import.v1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("blend", type=Path, help="Blender .blend source map")
    parser.add_argument("--map-id", help="Logical map id; defaults to source filename")
    parser.add_argument("--output", help="Runtime logical .ymap path; default maps/<map-id>.ymap")
    parser.add_argument("--cell-size", type=float, default=64.0)
    parser.add_argument("--origin", default="0,0,0", help="Map origin x,y,z in North Star coordinates")
    parser.add_argument("--root", type=Path, help="NorthStar repository root")
    parser.add_argument("--blender", type=Path, help="Blender executable override")
    parser.add_argument(
        "--no-build",
        action="store_true",
        help="Write/update authoring sources only; default behavior also compiles runtime YDD/YTYP/YMAP",
    )
    parser.add_argument("--dry-run", action="store_true", help="Inspect import result without replacing repository sources")
    return parser.parse_args()


def slug(value: str) -> str:
    import re

    value = re.sub(r"[^a-zA-Z0-9_]+", "_", value.strip()).strip("_").lower()
    return value or "map"


def repo_root(raw: Path | None) -> Path:
    if raw:
        root = raw.expanduser().resolve()
        if (root / "gameAssets").is_dir() and (root / "NewEngine" / "neocore2").is_dir():
            return root
        raise SystemExit(f"invalid NorthStar root: {root}")
    here = Path(__file__).resolve()
    for candidate in (Path.cwd().resolve(), *Path.cwd().resolve().parents, *here.parents):
        if (candidate / "gameAssets").is_dir() and (candidate / "NewEngine" / "neocore2").is_dir():
            return candidate
    raise SystemExit("NorthStar repository root not found; pass --root")


def parse_vec3(value: str) -> list[float]:
    parts = [float(part.strip()) for part in value.split(",")]
    if len(parts) != 3 or any(not math.isfinite(part) for part in parts):
        raise SystemExit(f"--origin must contain three finite numbers, got {value!r}")
    return parts


def find_blender(explicit: Path | None) -> Path:
    candidates: list[Path] = []
    if explicit:
        candidates.append(explicit.expanduser())
    for key in ("BLENDER_EXE", "BLENDER_PATH"):
        value = os.environ.get(key)
        if value:
            candidates.append(Path(value))
    found = shutil.which("blender")
    if found:
        candidates.append(Path(found))
    program_files = Path(os.environ.get("PROGRAMFILES", r"C:\Program Files"))
    blender_root = program_files / "Blender Foundation"
    if blender_root.is_dir():
        candidates.extend(sorted(blender_root.glob("Blender */blender.exe"), reverse=True))

    # Steam installs Blender outside Blender Foundation and do not necessarily
    # publish blender.exe into PATH. Probe both Program Files roots so the
    # one-command map import works for the common Steam installation as well.
    for env_key, fallback in (
        ("PROGRAMFILES", r"C:\Program Files"),
        ("PROGRAMFILES(X86)", r"C:\Program Files (x86)"),
    ):
        steam_blender = Path(os.environ.get(env_key, fallback)) / "Steam" / "steamapps" / "common" / "Blender" / "blender.exe"
        candidates.append(steam_blender)
    for candidate in candidates:
        candidate = candidate.resolve()
        if candidate.is_file():
            return candidate
    raise SystemExit("Blender executable not found; pass --blender or set BLENDER_EXE")


def fnv1a64(text: str) -> int:
    value = 0xCBF29CE484222325
    for byte in text.encode("utf-8"):
        value ^= byte
        value = (value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def format_vec(values: list[float]) -> str:
    return ",".join(f"{float(value):.9g}" for value in values)


def run_blender(blender: Path, blend: Path, exporter: Path, stage: Path) -> dict[str, Any]:
    manifest = stage / "blender_map_manifest.json"
    geometry_dir = stage / "geometry"
    command = [
        str(blender),
        "--background",
        str(blend),
        "--python",
        str(exporter),
        "--",
        "--manifest",
        str(manifest),
        "--geometry-dir",
        str(geometry_dir),
    ]
    print("[CMD]", subprocess.list2cmdline(command))
    result = subprocess.run(command, text=True, capture_output=True, timeout=900)
    if result.stdout.strip():
        print(result.stdout.rstrip())
    if result.stderr.strip():
        print(result.stderr.rstrip(), file=sys.stderr)
    if result.returncode:
        raise RuntimeError(f"Blender map export failed with code {result.returncode}")
    if not manifest.is_file():
        raise RuntimeError("Blender map exporter did not create manifest")
    data = json.loads(manifest.read_text(encoding="utf-8"))
    if data.get("schema") != "northstar.blender_map_export.v1":
        raise RuntimeError(f"unsupported Blender export manifest: {data.get('schema')!r}")
    return data


def write_definition_source(
    path: Path,
    *,
    definition_name: str,
    drawable_ref: str,
    material_ref: str,
    collision: bool,
) -> None:
    root = ET.Element(
        "YtypProperties",
        {
            "schema": "newengine.ytyp.properties.v1",
            "representation": "xml",
            "body_format": "newengine.xml.properties.v1",
            "name": definition_name,
            "kind": "map_static_mesh",
            "entry_kind": "archetype_definition",
            "stable_hash": str(fnv1a64(definition_name)),
            "flags": "0",
        },
    )
    dependencies = ET.SubElement(root, "Dependencies")
    ET.SubElement(
        dependencies,
        "Dependency",
        {
            "domain": "engine.model",
            "reference": drawable_ref,
            "role": "render/drawable",
            "required": "true",
        },
    )
    if material_ref:
        ET.SubElement(
            dependencies,
            "Dependency",
            {
                "domain": "engine.materials",
                "reference": material_ref,
                "role": "render/material",
                "required": "true",
            },
        )
        bindings = ET.SubElement(root, "MaterialBindings")
        ET.SubElement(
            bindings,
            "Binding",
            {"slot": "default", "material_ref": material_ref, "required": "true"},
        )
    semantic_tags = ET.SubElement(root, "SemanticTags")
    for tag in ("map", "static_mesh", "drawable"):
        ET.SubElement(semantic_tags, "Tag", {"value": tag})
    if collision:
        ET.SubElement(semantic_tags, "Tag", {"value": "collision"})
    domain_tags = ET.SubElement(root, "DomainTags")
    for tag in ("engine.assets.maps", "engine.assets.definitions", "engine.scene"):
        ET.SubElement(domain_tags, "Tag", {"value": tag})
    metadata = ET.SubElement(root, "Metadata")
    render = ET.SubElement(metadata, "Namespace", {"name": "render"})
    ET.SubElement(render, "Value", {"key": "mesh.role", "value": "world_static"})
    ET.SubElement(
        render,
        "Value",
        {"key": "collision.policy", "value": "static_mesh" if collision else "none"},
    )
    ET.SubElement(render, "Value", {"key": "streaming.policy", "value": "map_cell"})
    ET.indent(root, space="  ")
    path.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)


def cell_for(position: list[float], origin: list[float], cell_size: float) -> tuple[int, int]:
    return (
        math.floor((position[0] - origin[0]) / cell_size),
        math.floor((position[2] - origin[2]) / cell_size),
    )


def write_map_source(
    path: Path,
    *,
    map_id: str,
    cell_size: float,
    origin: list[float],
    instances: list[dict[str, Any]],
    generated_definition_refs: dict[str, str],
) -> tuple[int, int]:
    root = ET.Element(
        "YmapMapDefinition",
        {
            "schema": "newengine.map.definition.v2",
            "representation": "xml",
            "body_format": "newengine.xml.metadata.v1",
        },
    )
    map_node = ET.SubElement(
        root,
        "map",
        {"id": map_id, "cell_size": f"{cell_size:.9g}", "origin": format_vec(origin)},
    )
    cells_node = ET.SubElement(map_node, "cells")
    by_cell: dict[tuple[int, int], list[dict[str, Any]]] = {}
    for instance in instances:
        override = instance.get("cell_override")
        coord = tuple(override) if override is not None else cell_for(instance["position"], origin, cell_size)
        by_cell.setdefault((int(coord[0]), int(coord[1])), []).append(instance)

    placement_count = 0
    for coord in sorted(by_cell):
        cell_node = ET.SubElement(cells_node, "Cell", {"x": str(coord[0]), "z": str(coord[1])})
        placements = ET.SubElement(cell_node, "placements")
        for instance in sorted(by_cell[coord], key=lambda item: item["id"]):
            definition_ref = str(instance.get("definition_ref") or "").strip()
            if not definition_ref:
                asset_id = str(instance.get("asset_id") or "")
                definition_ref = generated_definition_refs[asset_id]
            placement = ET.SubElement(
                placements,
                "Placement",
                {
                    "id": instance["id"],
                    "definition_ref": definition_ref,
                    "position": format_vec(instance["position"]),
                    "rotation_ypr": format_vec(instance["rotation_ypr"]),
                    "scale": format_vec(instance["scale"]),
                    "apply_mode": instance.get("apply_mode") or "instantiate",
                    "enabled": "true" if instance.get("enabled", True) else "false",
                },
            )
            tags = instance.get("tags") or []
            if tags:
                tags_node = ET.SubElement(placement, "tags")
                for tag in sorted(set(str(tag).strip().lower() for tag in tags if str(tag).strip())):
                    ET.SubElement(tags_node, "Tag", {"value": tag})
            placement_count += 1

    ET.indent(root, space="  ")
    path.parent.mkdir(parents=True, exist_ok=True)
    ET.ElementTree(root).write(path, encoding="utf-8", xml_declaration=True)
    return len(by_cell), placement_count


def replace_records(records: list[dict[str, Any]], predicate, additions: list[dict[str, Any]], sort_key: str = "output") -> None:
    records[:] = [record for record in records if not predicate(record)]
    records.extend(additions)
    records.sort(key=lambda record: str(record.get(sort_key, "")))


def update_build_plan(
    plan_path: Path,
    *,
    map_id: str,
    output: str,
    model_records: list[dict[str, Any]],
    definition_records: list[dict[str, Any]],
    map_record: dict[str, Any],
) -> dict[str, Any]:
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    if plan.get("schema") != PLAN_SCHEMA:
        raise RuntimeError(f"unsupported build plan schema: {plan.get('schema')!r}")
    model_prefix = f"models/maps/{map_id}/"
    definition_prefix = f"definitions/maps/{map_id}/"
    replace_records(
        plan.setdefault("models", []),
        lambda record: str(record.get("output", "")).startswith(model_prefix),
        model_records,
    )
    replace_records(
        plan.setdefault("definitions", []),
        lambda record: str(record.get("output", "")).startswith(definition_prefix),
        definition_records,
    )
    replace_records(
        plan.setdefault("maps", []),
        lambda record: record.get("output") == output,
        [map_record],
    )
    return plan


def stable_id(value: str) -> str:
    return f"{fnv1a64(value):016x}"


def dependency_record(reference: str, role: str, domain: str, required: bool = True) -> dict[str, Any]:
    return {
        "reference": reference,
        "kind": role,
        "role": role,
        "domain": domain,
        "required": required,
    }


def asset_entry_manifest(
    logical_path: str,
    name: str,
    *,
    asset_kind: str,
    gateway: str,
    method: str,
    semantic_owner: str,
    dependencies: list[dict[str, Any]] | None = None,
    metadata: dict[str, str] | None = None,
) -> dict[str, Any]:
    entry_ref = f"{logical_path}@{name}"
    return {
        "name": name,
        "stable_id": stable_id(entry_ref),
        "asset_kind": asset_kind,
        "entry_ref": entry_ref,
        "route": {
            "gateway": gateway,
            "method": method,
            "semantic_owner": semantic_owner,
        },
        "dependencies": dependencies or [],
        "metadata": metadata or {},
    }


def ytyp_header_metadata(logical_path: str, source: Path) -> dict[str, Any]:
    root = ET.parse(source).getroot()
    name = str(root.attrib.get("name") or Path(logical_path).stem).strip()
    dependencies: list[dict[str, Any]] = []
    deps_node = root.find("Dependencies")
    if deps_node is not None:
        for node in deps_node.findall("Dependency"):
            reference = str(node.attrib.get("reference") or "").strip().replace("\\", "/")
            if not reference:
                continue
            role = str(node.attrib.get("role") or "dependency").strip()
            domain = str(node.attrib.get("domain") or "engine.assets.graph").strip()
            required = str(node.attrib.get("required") or "true").strip().lower() not in {"0", "false", "no", "off"}
            dependencies.append(dependency_record(reference, role, domain, required))
    return {
        "schema": "newengine.asset.list_file.header_metadata",
        "logical_path": logical_path,
        "content_kind": "ytyp_archetype_dictionary",
        "entries": [
            asset_entry_manifest(
                logical_path,
                name,
                asset_kind="archetype_definition",
                gateway="engine.assets.definitions",
                method="assets.definitions.entry_v1",
                semantic_owner="definition",
                dependencies=dependencies,
            )
        ],
        "dependencies": dependencies,
        "policy": [
            "YTYP entry semantics are owned by engine.assets.definitions",
            "runtime references use file.ytyp@entry",
        ],
    }


def ymap_header_metadata(logical_path: str, source: Path) -> dict[str, Any]:
    root = ET.parse(source).getroot()
    map_node = root.find("map")
    if map_node is None:
        raise RuntimeError(f"generated YMAP has no <map>: {source}")
    entries: list[dict[str, Any]] = []
    map_dependencies: list[dict[str, Any]] = []
    all_definition_dependencies: dict[str, dict[str, Any]] = {}
    cells_node = map_node.find("cells")
    if cells_node is not None:
        for cell_node in cells_node.findall("Cell"):
            x = int(cell_node.attrib["x"])
            z = int(cell_node.attrib["z"])
            cell_name = f"cell/{x}/{z}"
            cell_dependencies: list[dict[str, Any]] = []
            placements = cell_node.find("placements")
            if placements is not None:
                for placement in placements.findall("Placement"):
                    reference = str(placement.attrib.get("definition_ref") or "").strip().replace("\\", "/")
                    if not reference:
                        continue
                    dependency = dependency_record(
                        reference,
                        "definition",
                        "engine.assets.definitions",
                        True,
                    )
                    if reference not in {item["reference"] for item in cell_dependencies}:
                        cell_dependencies.append(dependency)
                    all_definition_dependencies.setdefault(reference, dependency)
            entries.append(
                asset_entry_manifest(
                    logical_path,
                    cell_name,
                    asset_kind="map_cell",
                    gateway="engine.assets.maps",
                    method="assets.maps.cell_v1",
                    semantic_owner="map_cell",
                    dependencies=cell_dependencies,
                    metadata={"cell_x": str(x), "cell_z": str(z)},
                )
            )
            map_dependencies.append(
                dependency_record(
                    f"{logical_path}@{cell_name}",
                    "map_cell",
                    "engine.assets.maps",
                    True,
                )
            )
    entries.insert(
        0,
        asset_entry_manifest(
            logical_path,
            "map",
            asset_kind="map_index",
            gateway="engine.assets.maps",
            method="assets.maps.index_v1",
            semantic_owner="map",
            dependencies=map_dependencies,
            metadata={
                "map_id": str(map_node.attrib.get("id") or ""),
                "cell_size": str(map_node.attrib.get("cell_size") or ""),
            },
        ),
    )
    return {
        "schema": "newengine.asset.list_file.header_metadata",
        "logical_path": logical_path,
        "content_kind": "ymap_map_data",
        "entries": entries,
        "dependencies": sorted(all_definition_dependencies.values(), key=lambda item: item["reference"]),
        "policy": [
            "YMAP owns map index/cell composition only",
            "placements resolve through .ytyp@entry",
            "map/cell entries are addressed as file.ymap@map and file.ymap@cell/x/z",
        ],
    }


def write_nef8(
    output: Path,
    body: bytes,
    *,
    content_kind: int,
    schema_version: int,
    entry_count: int,
    header_metadata: dict[str, Any] | None = None,
) -> None:
    """Write a canonical class-5 NEF8 envelope atomically."""
    compressor = zlib.compressobj(level=9, wbits=-zlib.MAX_WBITS)
    stored = compressor.compress(body) + compressor.flush()
    metadata_bytes = (
        json.dumps(header_metadata, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if header_metadata
        else b""
    )
    header = bytearray(32)
    header[0:4] = b"NEF8"
    header[4] = 2
    header[5] = 5
    struct.pack_into("<H", header, 6, content_kind)
    flags = 0x0001 | (0x0002 if metadata_bytes else 0)
    struct.pack_into("<H", header, 8, flags)
    struct.pack_into("<H", header, 10, schema_version)
    struct.pack_into("<I", header, 12, entry_count)
    struct.pack_into("<Q", header, 16, len(stored))
    struct.pack_into("<Q", header, 24, len(body))
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.importing")
    temporary.write_bytes(bytes(header) + metadata_bytes + stored)
    os.replace(temporary, output)


def ydd_packer(repo: Path) -> Path:
    candidates = [
        repo / "tools" / "northstar-ydd-packer.exe",
        repo / "NewEngine" / "tools" / "northstar-ydd-packer.exe",
    ]
    found = shutil.which("northstar-ydd-packer")
    if found:
        candidates.append(Path(found))
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise RuntimeError(
        "northstar-ydd-packer was not found; expected repository tools/northstar-ydd-packer.exe"
    )



def decode_legacy_packer_ydd(path: Path) -> dict[str, Any]:
    """Decode the legacy YDD envelope emitted by the repository's old packer.

    The old tool uses NEF8 v1 with a u16 header length and a JSON runtime-mesh
    body. Runtime AssetManager now requires the canonical size-class NEF8 v2
    envelope and strict binary mesh v2 body, so the importer upgrades it before
    publishing the asset.
    """
    data = path.read_bytes()
    if len(data) < 40 or data[:4] != b"NEF8":
        raise RuntimeError(f"legacy YDD packer output is not NEF8 path='{path}'")
    version = struct.unpack_from("<H", data, 4)[0]
    header_len = struct.unpack_from("<H", data, 6)[0]
    content_kind = struct.unpack_from("<H", data, 8)[0]
    compression = struct.unpack_from("<H", data, 10)[0]
    body_offset = struct.unpack_from("<Q", data, 16)[0]
    body_len = struct.unpack_from("<Q", data, 24)[0]
    body_uncompressed_len = struct.unpack_from("<Q", data, 32)[0]
    if version != 1 or content_kind != 2 or compression != 1:
        raise RuntimeError(
            f"unsupported legacy YDD envelope path='{path}' version={version} "
            f"content_kind={content_kind} compression={compression}"
        )
    if header_len != body_offset or body_offset + body_len > len(data):
        raise RuntimeError(f"invalid legacy YDD body range path='{path}'")
    try:
        raw = zlib.decompress(data[body_offset : body_offset + body_len], wbits=-zlib.MAX_WBITS)
    except zlib.error as error:
        raise RuntimeError(f"legacy YDD deflate decode failed path='{path}': {error}") from error
    if body_uncompressed_len and len(raw) != body_uncompressed_len:
        raise RuntimeError(
            f"legacy YDD body length mismatch path='{path}' "
            f"decoded={len(raw)} expected={body_uncompressed_len}"
        )
    try:
        document = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError(f"legacy YDD JSON body decode failed path='{path}': {error}") from error
    if document.get("schema") != "newengine.ydd.runtime_mesh_parts.v1":
        raise RuntimeError(
            f"legacy YDD body schema unsupported path='{path}' schema={document.get('schema')!r}"
        )
    return document


def encode_ydd_binary_v2(document: dict[str, Any]) -> bytes:
    """Encode the strict `newengine.ydd.binary_mesh.v2` body consumed by runtime."""
    entry_defs = document.get("entries") or []
    mesh_parts = document.get("runtime_mesh_parts") or []
    if not isinstance(entry_defs, list) or not entry_defs:
        raise RuntimeError("legacy YDD document contains no entries")
    if not isinstance(mesh_parts, list) or not mesh_parts:
        raise RuntimeError("legacy YDD document contains no runtime mesh parts")

    by_entry: dict[str, list[dict[str, Any]]] = {}
    for mesh in mesh_parts:
        by_entry.setdefault(str(mesh.get("entry") or ""), []).append(mesh)

    strings = bytearray()
    offsets: dict[str, int] = {}

    def intern(value: str | None) -> int:
        if value is None or not str(value):
            return 0xFFFFFFFF
        text = str(value)
        if text in offsets:
            return offsets[text]
        offset = len(strings)
        strings.extend(text.encode("utf-8"))
        strings.append(0)
        offsets[text] = offset
        return offset

    # Freeze the string table before payload offsets are calculated.
    for entry in entry_defs:
        entry_name = str(entry.get("name") or "")
        if not entry_name:
            raise RuntimeError("legacy YDD entry is missing a name")
        intern(entry_name)
        intern(str(entry.get("source_path") or ""))
        for mesh in by_entry.get(entry_name, []):
            intern(str(mesh.get("name") or entry_name))
            material_ref = mesh.get("material_ref")
            if material_ref:
                intern(str(material_ref))

    def vec3(values: Any, label: str) -> tuple[float, float, float]:
        if not isinstance(values, (list, tuple)) or len(values) != 3:
            raise RuntimeError(f"binary YDD {label} must be vec3, got {values!r}")
        result = tuple(float(value) for value in values)
        if any(not math.isfinite(value) for value in result):
            raise RuntimeError(f"binary YDD {label} contains non-finite values")
        return result  # type: ignore[return-value]

    def vec2(values: Any, label: str) -> tuple[float, float]:
        if not isinstance(values, (list, tuple)) or len(values) != 2:
            raise RuntimeError(f"binary YDD {label} must be vec2, got {values!r}")
        result = tuple(float(value) for value in values)
        if any(not math.isfinite(value) for value in result):
            raise RuntimeError(f"binary YDD {label} contains non-finite values")
        return result  # type: ignore[return-value]

    payloads: list[bytes] = []
    entry_summaries: list[tuple[dict[str, Any], list[dict[str, Any]], int, int]] = []
    for entry in entry_defs:
        entry_name = str(entry.get("name") or "")
        meshes = by_entry.get(entry_name, [])
        if not meshes:
            raise RuntimeError(f"legacy YDD entry has no mesh payload entry='{entry_name}'")
        payload = bytearray(struct.pack("<II", len(meshes), 0))
        total_vertices = 0
        total_indices = 0
        for mesh in meshes:
            vertices = mesh.get("vertices") or []
            indices = mesh.get("indices") or []
            if not vertices or not indices or len(indices) % 3:
                raise RuntimeError(
                    f"legacy YDD mesh must contain triangular geometry entry='{entry_name}' "
                    f"mesh='{mesh.get('name')}' vertices={len(vertices)} indices={len(indices)}"
                )
            positions = [vec3(vertex.get("pos"), "vertex.position") for vertex in vertices]
            bounds_min = tuple(min(position[axis] for position in positions) for axis in range(3))
            bounds_max = tuple(max(position[axis] for position in positions) for axis in range(3))
            mesh_name = str(mesh.get("name") or entry_name)
            material_ref = mesh.get("material_ref")
            payload.extend(
                struct.pack(
                    "<IIIIffffff",
                    intern(mesh_name),
                    intern(str(material_ref)) if material_ref else 0xFFFFFFFF,
                    len(vertices),
                    len(indices),
                    *bounds_min,
                    *bounds_max,
                )
            )
            for vertex in vertices:
                position = vec3(vertex.get("pos"), "vertex.position")
                normal = vec3(vertex.get("nrm"), "vertex.normal")
                uv = vec2(vertex.get("uv"), "vertex.uv")
                payload.extend(struct.pack("<ffffffff", *position, *normal, *uv))
            for index in indices:
                index_value = int(index)
                if index_value < 0 or index_value >= len(vertices):
                    raise RuntimeError(
                        f"legacy YDD index out of bounds entry='{entry_name}' "
                        f"mesh='{mesh_name}' index={index_value} vertices={len(vertices)}"
                    )
                payload.extend(struct.pack("<I", index_value))
            total_vertices += len(vertices)
            total_indices += len(indices)
        payloads.append(bytes(payload))
        entry_summaries.append((entry, meshes, total_vertices, total_indices))

    body_header_len = 40
    entry_record_len = 80
    table_offset = body_header_len
    string_offset = table_offset + len(entry_defs) * entry_record_len
    payload_floor = string_offset + len(strings)
    payload_offsets: list[int] = []
    cursor = payload_floor
    for payload in payloads:
        payload_offsets.append(cursor)
        cursor += len(payload)

    body = bytearray(cursor)
    struct.pack_into(
        "<IIQQQQ",
        body,
        0,
        2,  # YDD_BINARY_SCHEMA_VERSION
        len(entry_defs),
        table_offset,
        string_offset,
        len(strings),
        payload_floor,
    )
    body[string_offset : string_offset + len(strings)] = strings

    for index, ((entry, meshes, total_vertices, total_indices), payload) in enumerate(
        zip(entry_summaries, payloads)
    ):
        record = table_offset + index * entry_record_len
        entry_name = str(entry.get("name") or "")
        bounds_min = vec3(entry.get("bounds_min"), "entry.bounds_min")
        bounds_max = vec3(entry.get("bounds_max"), "entry.bounds_max")
        struct.pack_into("<Q", body, record + 0, 0)
        struct.pack_into("<I", body, record + 8, intern(entry_name))
        struct.pack_into("<I", body, record + 12, intern(str(entry.get("source_path") or "")))
        struct.pack_into("<I", body, record + 16, len(meshes))
        struct.pack_into("<I", body, record + 20, total_vertices)
        struct.pack_into("<I", body, record + 24, total_indices)
        struct.pack_into("<I", body, record + 28, 0)
        struct.pack_into("<I", body, record + 32, 0xFFFFFFFF)
        struct.pack_into("<fff", body, record + 36, *bounds_min)
        struct.pack_into("<fff", body, record + 48, *bounds_max)
        struct.pack_into("<I", body, record + 60, 0)
        struct.pack_into("<Q", body, record + 64, payload_offsets[index])
        struct.pack_into("<Q", body, record + 72, len(payload))
        start = payload_offsets[index]
        body[start : start + len(payload)] = payload

    return bytes(body)


def upgrade_legacy_packer_ydd(path: Path) -> None:
    document = decode_legacy_packer_ydd(path)
    body = encode_ydd_binary_v2(document)
    write_nef8(
        path,
        body,
        content_kind=2,
        schema_version=2,
        entry_count=len(document.get("entries") or []),
    )


def compile_ydd(repo: Path, source: Path, output: Path) -> None:
    packer = ydd_packer(repo)
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.stem}.importing.ydd")
    temporary.unlink(missing_ok=True)
    command = [str(packer), "pack", "--input", str(source), "--output", str(temporary)]
    print("[CMD]", subprocess.list2cmdline(command))
    result = subprocess.run(command, cwd=repo, text=True, capture_output=True, timeout=900)
    if result.stdout.strip():
        print(result.stdout.rstrip())
    if result.stderr.strip():
        print(result.stderr.rstrip(), file=sys.stderr)
    if result.returncode or not temporary.is_file():
        temporary.unlink(missing_ok=True)
        raise RuntimeError(f"YDD compile failed output='{output}' code={result.returncode}")
    upgrade_legacy_packer_ydd(temporary)
    os.replace(temporary, output)


def prune_generated_directory(directory: Path, keep_names: set[str]) -> None:
    if not directory.is_dir():
        return
    for path in directory.iterdir():
        if path.is_file() and path.name not in keep_names:
            path.unlink(missing_ok=True)
    try:
        directory.rmdir()
    except OSError:
        pass


def compile_generated_runtime(
    repo: Path,
    asset_root: Path,
    *,
    model_records: list[dict[str, Any]],
    staged_definitions: list[tuple[Path, Path]],
    definition_records: list[dict[str, Any]],
    staged_map: Path,
    map_output_rel: str,
    map_entry_count: int,
) -> None:
    for record in model_records:
        compile_ydd(repo, asset_root / record["source"], asset_root / record["output"])

    source_by_name = {destination.name: staged for staged, destination in staged_definitions}
    for record in definition_records:
        source_name = Path(record["source"]).name
        source = source_by_name[source_name]
        write_nef8(
            asset_root / record["output"],
            source.read_bytes(),
            content_kind=3,  # LIST_FILE_CONTENT_KIND_YTYP
            schema_version=1,
            entry_count=1,
            header_metadata=ytyp_header_metadata(record["output"], source),
        )

    write_nef8(
        asset_root / map_output_rel,
        staged_map.read_bytes(),
        content_kind=5,  # LIST_FILE_CONTENT_KIND_YMAP
        schema_version=2,
        entry_count=map_entry_count,
        header_metadata=ymap_header_metadata(map_output_rel, staged_map),
    )


def main() -> int:
    options = parse_args()
    repo = repo_root(options.root)
    blend = options.blend.expanduser().resolve()
    if not blend.is_file() or blend.suffix.lower() != ".blend":
        raise SystemExit(f"Blender map source must be an existing .blend file: {blend}")
    if not math.isfinite(options.cell_size) or options.cell_size <= 0.0:
        raise SystemExit("--cell-size must be finite and > 0")

    map_id = slug(options.map_id or blend.stem)
    output = (options.output or f"maps/{map_id}.ymap").replace("\\", "/").lstrip("/")
    if not output.lower().endswith(".ymap"):
        raise SystemExit("--output must end with .ymap")
    origin = parse_vec3(options.origin)
    asset_root = repo / "gameAssets"
    plan_path = repo / "tools" / "asset_manifests" / "native_asset_build_plan.v1.json"
    exporter = repo / "NewEngine" / "neocore2" / "scripts" / "blender_map_export.py"
    blender = find_blender(options.blender)

    with tempfile.TemporaryDirectory(prefix=f"northstar-blender-map-{map_id}-") as temp:
        stage = Path(temp)
        exported = run_blender(blender, blend, exporter, stage)
        assets = exported.get("assets", [])
        instances = exported.get("instances", [])

        model_records: list[dict[str, Any]] = []
        model_stage_sources: list[tuple[Path, Path]] = []
        generated_definition_refs: dict[str, str] = {}
        definition_records: list[dict[str, Any]] = []
        definition_sources: list[tuple[Path, dict[str, Any]]] = []
        for asset in sorted(assets, key=lambda item: item["asset_id"]):
            asset_id = asset["asset_id"]
            geometry_stage = Path(str(asset.get("geometry_file") or ""))
            if not geometry_stage.is_file():
                raise RuntimeError(
                    f"Blender exporter produced no geometry for asset '{asset_id}': {geometry_stage}"
                )
            model_source_rel = f"models/source/maps/{map_id}/{asset_id}.obj"
            model_output_rel = f"models/maps/{map_id}/{asset_id}.ydd"
            model_records.append({"source": model_source_rel, "output": model_output_rel})
            model_stage_sources.append((geometry_stage, asset_root / model_source_rel))

            definition_source_rel = f"definitions/source/maps/{map_id}/{asset_id}.ytyp.xml"
            definition_output_rel = f"definitions/maps/{map_id}/{asset_id}.ytyp"
            definition_ref = f"{definition_output_rel}@{asset_id}"
            generated_definition_refs[asset_id] = definition_ref
            definition_records.append({"source": definition_source_rel, "output": definition_output_rel})
            definition_sources.append(
                (
                    asset_root / definition_source_rel,
                    {
                        "definition_name": asset_id,
                        "drawable_ref": f"{model_output_rel}@{asset_id}",
                        "material_ref": str(asset.get("material_ref") or ""),
                        "collision": bool(asset.get("collision", False)),
                    },
                )
            )

        map_source_rel = f"maps/source/imported/{map_id}.ymap.xml"
        map_source = asset_root / map_source_rel
        map_record = {"source": map_source_rel, "output": output, "logical_path": output}

        # Render generated source into staging first, then replace repository files.
        staged_map = stage / f"{map_id}.ymap.xml"
        cell_count, placement_count = write_map_source(
            staged_map,
            map_id=map_id,
            cell_size=options.cell_size,
            origin=origin,
            instances=instances,
            generated_definition_refs=generated_definition_refs,
        )
        staged_definitions: list[tuple[Path, Path]] = []
        for destination, payload in definition_sources:
            staged = stage / "definitions" / destination.name
            write_definition_source(staged, **payload)
            staged_definitions.append((staged, destination))

        updated_plan = update_build_plan(
            plan_path,
            map_id=map_id,
            output=output,
            model_records=model_records,
            definition_records=definition_records,
            map_record=map_record,
        )
        receipt = {
            "schema": IMPORT_SCHEMA,
            "map_id": map_id,
            "source_blend": str(blend),
            "source_sha256": sha256(blend),
            "output": output,
            "cell_size": options.cell_size,
            "origin": origin,
            "unique_mesh_assets": len(assets),
            "instances": len(instances),
            "cells": cell_count,
            "placements": placement_count,
            "replacement_policy": "same logical .ymap output is atomically replaced in-place; generated YDD/YTYP/YMAP paths are stable per map id",
            "runtime_build": not options.no_build,
            "generated": {
                "models": [record["output"] for record in model_records],
                "definitions": [record["output"] for record in definition_records],
                "map": output,
            },
        }

        if options.dry_run:
            print(json.dumps(receipt, indent=2, ensure_ascii=False))
            return 0

        for staged_geometry, destination in model_stage_sources:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(staged_geometry, destination)
        for staged, destination in staged_definitions:
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(staged, destination)
        map_source.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(staged_map, map_source)
        if not options.no_build:
            # Dependencies are compiled first and the .ymap is atomically replaced last.
            # A failed import therefore never publishes a map that points at missing assets.
            compile_generated_runtime(
                repo,
                asset_root,
                model_records=model_records,
                staged_definitions=staged_definitions,
                definition_records=definition_records,
                staged_map=staged_map,
                map_output_rel=output,
                map_entry_count=cell_count + 1,
            )

        # Build-plan/receipt become authoritative only after source generation and,
        # unless --no-build was requested, runtime compilation succeeded.
        plan_path.write_text(json.dumps(updated_plan, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        receipt_path = asset_root / "maps" / "source" / "imported" / f"{map_id}.import.json"
        receipt_path.write_text(json.dumps(receipt, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

        source_model_names = {Path(record["source"]).name for record in model_records}
        source_definition_names = {Path(record["source"]).name for record in definition_records}
        prune_generated_directory(asset_root / "models" / "source" / "maps" / map_id, source_model_names)
        prune_generated_directory(
            asset_root / "definitions" / "source" / "maps" / map_id,
            source_definition_names,
        )
        if not options.no_build:
            runtime_model_names = {Path(record["output"]).name for record in model_records}
            runtime_definition_names = {Path(record["output"]).name for record in definition_records}
            prune_generated_directory(asset_root / "models" / "maps" / map_id, runtime_model_names)
            prune_generated_directory(
                asset_root / "definitions" / "maps" / map_id,
                runtime_definition_names,
            )

        print(
            "BLENDER_MAP_IMPORT_OK "
            f"map='{output}' cells={cell_count} placements={placement_count} assets={len(assets)} build={not options.no_build}"
        )
        print(f"[MAP SOURCE] {map_source}")
        print(f"[IMPORT RECEIPT] {receipt_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
