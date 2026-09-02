#!/usr/bin/env python3
"""NorthStar provider-side SpeedTree Modeler (.spm) importer.

Production policy:
    SPM -> SpeedTree Modeler command-line generator -> SpeedTreeRaw XML
        -> native NEF8/YDD LODs + texture source set + .nefoliage metadata.

The engine core never links the SpeedTree SDK and never decodes proprietary SPM
internals.  When a licensed SpeedTree Modeler is available, this importer asks the
actual Modeler to evaluate the procedural generator graph.  The old NorthStar
node-graph approximation is deliberately not a production fallback because it
cannot reproduce authored branch profiles, leaf meshes, LOD, wind data or
collision.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import math
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from import_openformats_vegetation import Entry, Mesh, Vertex, encode_nef8_ydd, encode_ydd_body

IMPORTER_ID = "northstar.importer.speedtree_spm.v1"
RUNTIME_SCHEMA = "newengine.foliage.runtime.source.v1"
DEFAULT_SPEEDTREE_ROOT = Path(r"C:\Program Files\SpeedTree\SpeedTree Modeler v10.0.1")
RUNTIME_TEXTURE_MAX_DIM = 1536

MATERIAL_NAMES = {
    5: "oak_bark",
    13: "oak_cluster_branch",
    14: "oak_leaf",
    23456: "oak_atlas",
}


@dataclass(frozen=True)
class SpmDocument:
    source: Path
    xml_root: ET.Element
    modeler_version: str
    declared_triangles: int
    species: str


@dataclass(frozen=True)
class SpeedTreeGenerated:
    root: ET.Element
    export_dir: Path
    source_height: float
    source_min_z: float
    source_max_z: float
    collision: tuple[dict[str, object], ...]


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def read_spm(path: Path) -> SpmDocument:
    raw = path.read_bytes()
    if len(raw) < 2 or raw[:2] != b"\x1f\x8b":
        raise ValueError(f"{path}: expected GZip SpeedTree Modeler source")
    try:
        root = ET.fromstring(gzip.decompress(raw))
    except (OSError, ET.ParseError) as error:
        raise ValueError(f"{path}: invalid SpeedTree SPM payload: {error}") from error
    if root.tag.split("}")[-1] != "SpeedTree":
        raise ValueError(f"{path}: XML root is not SpeedTree")

    declared_triangles = 0
    try:
        declared_triangles = int(root.findtext(".//TotalTriangles", default="0"))
    except ValueError:
        pass

    species_values: list[str] = []
    for item in root.findall(".//TreeInfo/Item"):
        if item.attrib.get("Key") in {"Names", "ScientificNames"}:
            value = item.attrib.get("Value", "").strip()
            if value and value not in species_values:
                species_values.append(value)
    return SpmDocument(
        source=path,
        xml_root=root,
        modeler_version=root.attrib.get("VersionString", "").strip(),
        declared_triangles=declared_triangles,
        species=" / ".join(species_values),
    )


def _modeler_executable(speedtree_root: Path) -> Path:
    candidates = (
        speedtree_root / "win64" / "SpeedTree_Modeler.exe",
        speedtree_root / "SpeedTree_Modeler.exe",
    )
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise FileNotFoundError(
        f"SpeedTree Modeler executable not found under {speedtree_root}; "
        "production SPM generation requires a licensed Modeler provider"
    )


def _runtime_sdk_export_preset(speedtree_root: Path) -> Path:
    preset = speedtree_root / "export_presets" / "Games" / "_SpeedTree Runtime SDK.ini"
    if not preset.is_file():
        raise FileNotFoundError(f"SpeedTree Runtime SDK export preset not found: {preset}")
    return preset


def export_generated_speedtree(document: SpmDocument, speedtree_root: Path, export_dir: Path) -> SpeedTreeGenerated:
    executable = _modeler_executable(speedtree_root)
    preset = _runtime_sdk_export_preset(speedtree_root)
    export_dir.mkdir(parents=True, exist_ok=True)
    raw_xml = export_dir / f"{document.source.stem}.speedtree.raw.xml"
    raw_xml.unlink(missing_ok=True)
    command = [
        str(executable),
        str(document.source),
        "-export_game",
        str(raw_xml),
        "-export_options",
        str(preset),
    ]
    completed = subprocess.run(
        command,
        cwd=str(export_dir),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        timeout=120,
    )
    if completed.returncode != 0 or not raw_xml.is_file():
        detail = completed.stdout.strip()[-4000:]
        raise RuntimeError(
            "SpeedTree Modeler command-line generation failed. "
            "A Pro/Enterprise-capable license is required for command-line export. "
            f"exit={completed.returncode} output={detail!r}"
        )

    root = ET.parse(raw_xml).getroot()
    if root.tag != "SpeedTreeRaw":
        raise ValueError(f"{raw_xml}: expected SpeedTreeRaw root, got {root.tag!r}")
    objects = root.find("Objects")
    if objects is None:
        raise ValueError(f"{raw_xml}: missing Objects")
    min_z = float(objects.attrib.get("BoundsMinZ", "0"))
    max_z = float(objects.attrib.get("BoundsMaxZ", "0"))
    # Ground is Z=0 in authored SpeedTree space. Preserve root geometry below ground,
    # but define requested tree height from ground to the highest generated vertex.
    source_height = max_z
    if not math.isfinite(source_height) or source_height <= 0.01:
        raise ValueError(f"{raw_xml}: invalid generated height {source_height}")

    collision: list[dict[str, object]] = []
    collision_root = root.find("CollisionObjects")
    if collision_root is not None:
        for item in collision_root.findall("CollisionObject"):
            kind = item.attrib.get("Type", "").strip().lower()
            if kind not in {"capsule", "sphere"}:
                continue
            collision.append(
                {
                    "type": kind,
                    "p1": [
                        float(item.attrib.get("Pos1X", "0")),
                        float(item.attrib.get("Pos1Y", "0")),
                        float(item.attrib.get("Pos1Z", "0")),
                    ],
                    "p2": [
                        float(item.attrib.get("Pos2X", "0")),
                        float(item.attrib.get("Pos2Y", "0")),
                        float(item.attrib.get("Pos2Z", "0")),
                    ],
                    "radius": float(item.attrib.get("Radius", "0")),
                }
            )
    return SpeedTreeGenerated(root, export_dir, source_height, min_z, max_z, tuple(collision))


def _float_array(parent: ET.Element, name: str) -> list[float]:
    element = parent.find(name)
    if element is None:
        raise ValueError(f"missing SpeedTreeRaw stream {name}")
    return [float(value) for value in (element.text or "").split()]


def _int_array(parent: ET.Element, name: str) -> list[int]:
    element = parent.find(name)
    if element is None:
        raise ValueError(f"missing SpeedTreeRaw stream {name}")
    return [int(value) for value in (element.text or "").split()]


def _convert_position(x: float, y: float, z: float, scale: float) -> tuple[float, float, float]:
    # SpeedTree generated data is Z-up. NorthStar is Y-up.
    return (x * scale, z * scale, -y * scale)


def _convert_direction(x: float, y: float, z: float) -> tuple[float, float, float]:
    length = math.sqrt(max(1.0e-20, x * x + y * y + z * z))
    return (x / length, z / length, -y / length)


def build_exact_lod_entries(
    generated: SpeedTreeGenerated,
    entry_base: str,
    target_height: float,
    logical_source: str,
    material_library_ref: str,
    lod0_exclude_materials: set[int] | None = None,
) -> list[Entry]:
    objects_root = generated.root.find("Objects")
    if objects_root is None:
        raise ValueError("SpeedTreeRaw Objects missing")
    objects = objects_root.findall("Object")
    by_id = {item.attrib.get("ID", ""): item for item in objects}
    scale = target_height / generated.source_height
    entries: list[Entry] = []

    for lod_index in range(3):
        lod_name = f"LOD{lod_index}"
        lod_root = next((item for item in objects if item.attrib.get("Name") == lod_name), None)
        if lod_root is None:
            raise ValueError(f"SpeedTreeRaw missing {lod_name}")
        lod_id = lod_root.attrib.get("ID", "")

        # Merge generated Branches/Leaves by SpeedTree material while preserving
        # the exact point-index / vertex-index relationship produced by Modeler.
        builders: dict[int, tuple[list[Vertex], list[int]]] = {}
        for obj in objects:
            if obj.attrib.get("ParentID") != lod_id:
                continue
            points = obj.find("Points")
            attrs = obj.find("Vertices")
            if points is None or attrs is None:
                continue
            px, py, pz = (_float_array(points, key) for key in ("X", "Y", "Z"))
            nx, ny, nz = (_float_array(attrs, key) for key in ("NormalX", "NormalY", "NormalZ"))
            uu, vv = (_float_array(attrs, key) for key in ("TexcoordU", "TexcoordV"))
            point_count = len(px)
            vertex_count = len(nx)
            if not (len(py) == len(pz) == point_count and len(ny) == len(nz) == len(uu) == len(vv) == vertex_count):
                raise ValueError(f"{lod_name}/{obj.attrib.get('Name')}: inconsistent SpeedTreeRaw streams")

            for triangles in obj.findall("Triangles"):
                material_id = int(triangles.attrib.get("Material", "0"))
                if lod_index == 0 and material_id in (lod0_exclude_materials or set()):
                    continue
                point_indices = _int_array(triangles, "PointIndices")
                vertex_indices = _int_array(triangles, "VertexIndices")
                if len(point_indices) != len(vertex_indices) or len(point_indices) % 3:
                    raise ValueError(f"{lod_name}: malformed triangle streams for material {material_id}")
                vertices, indices = builders.setdefault(material_id, ([], []))
                remap: dict[tuple[int, int], int] = {}
                for point_index, vertex_index in zip(point_indices, vertex_indices):
                    if point_index < 0 or point_index >= point_count or vertex_index < 0 or vertex_index >= vertex_count:
                        raise ValueError(f"{lod_name}: SpeedTreeRaw index out of range")
                    key = (point_index, vertex_index)
                    mapped = remap.get(key)
                    if mapped is None:
                        mapped = len(vertices)
                        remap[key] = mapped
                        vertices.append(
                            Vertex(
                                _convert_position(px[point_index], py[point_index], pz[point_index], scale),
                                _convert_direction(nx[vertex_index], ny[vertex_index], nz[vertex_index]),
                                (uu[vertex_index], vv[vertex_index]),
                            )
                        )
                    indices.append(mapped)

        meshes: list[Mesh] = []
        for material_id in sorted(builders):
            vertices, indices = builders[material_id]
            if not vertices or not indices:
                continue
            material_name = MATERIAL_NAMES.get(material_id, "oak_atlas")
            meshes.append(
                Mesh(
                    name=f"{entry_base}_lod{lod_index}_{material_name}",
                    material_ref=f"{material_library_ref}@{material_name}",
                    vertices=vertices,
                    indices=indices,
                )
            )
        if not meshes:
            raise ValueError(f"SpeedTreeRaw {lod_name} produced no runtime meshes")
        entries.append(Entry(name=f"{entry_base}_lod{lod_index}", source_path=logical_source, meshes=meshes))
    return entries


def write_ydd(entries: list[Entry], output: Path) -> None:
    body = encode_ydd_body(entries)
    encoded = encode_nef8_ydd(body, len(entries))
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)


def _resolve_spm_texture(document: SpmDocument, raw_value: str, speedtree_root: Path) -> Path | None:
    value = raw_value.strip().replace("\\", "/")
    if not value:
        return None
    direct = Path(value)
    if direct.is_file():
        return direct
    relative = (document.source.parent / direct).resolve()
    if relative.is_file():
        return relative
    marker = "Program Files/"
    pos = value.lower().find(marker.lower())
    if pos >= 0:
        candidate = Path("C:/") / value[pos:]
        if candidate.is_file():
            return candidate
    # Last-resort lookup only inside the installed Modeler sample tree.
    matches = list((speedtree_root / "samples").rglob(Path(value).name))
    return matches[0] if matches else None


def _spm_material_maps(document: SpmDocument, speedtree_root: Path) -> dict[int, dict[str, Path]]:
    output: dict[int, dict[str, Path]] = {}
    for material in document.xml_root.iter():
        if material.tag.split("}")[-1] != "Material_v8":
            continue
        try:
            material_id = int(material.attrib.get("ID", "-1"))
        except ValueError:
            continue
        maps: dict[str, Path] = {}
        for map_node in material.findall(".//Map"):
            name = map_node.attrib.get("Name", "").strip().lower()
            filename = map_node.findtext("TexFilename", default="").strip()
            resolved = _resolve_spm_texture(document, filename, speedtree_root)
            if name and resolved is not None:
                maps[name] = resolved
        if maps:
            output[material_id] = maps
    return output


def _runtime_safe_image(image, max_dim: int = RUNTIME_TEXTURE_MAX_DIM):
    from PIL import Image

    width, height = image.size
    longest = max(width, height)
    if longest <= max_dim:
        return image.copy()
    scale = max_dim / float(longest)
    target = (max(1, round(width * scale)), max(1, round(height * scale)))
    return image.resize(target, Image.Resampling.LANCZOS)


def _copy_runtime_image(source: Path, output: Path, mode: str | None = None) -> None:
    from PIL import Image

    with Image.open(source) as image:
        converted = image.convert(mode) if mode is not None else image.copy()
        runtime = _runtime_safe_image(converted)
        runtime.save(output)


def _bleed_transparent_rgb(image, max_distance: float = 18.0):
    """Fill transparent RGB from the nearest authored opaque texel, keep alpha intact.

    Ordinary RGBA mip generation averages transparent black into leaf colors. The
    alpha channel may still pass a masked cutoff while RGB has collapsed toward
    black, which produces unstable fringes. Nearest-color bleed is the standard
    cutout-texture preparation step and is deterministic for a fixed source.
    """
    try:
        import numpy as np
        from scipy import ndimage
    except ImportError:
        return image.copy()

    rgba = np.asarray(image.convert("RGBA"), dtype=np.uint8).copy()
    alpha = rgba[..., 3]
    authored = alpha >= 8
    if not authored.any() or authored.all():
        return image.copy()
    distance, indices = ndimage.distance_transform_edt(~authored, return_indices=True)
    fill = (~authored) & (distance <= max_distance)
    nearest_y = indices[0][fill]
    nearest_x = indices[1][fill]
    rgba[fill, 0] = rgba[nearest_y, nearest_x, 0]
    rgba[fill, 1] = rgba[nearest_y, nearest_x, 1]
    rgba[fill, 2] = rgba[nearest_y, nearest_x, 2]
    from PIL import Image
    return Image.fromarray(rgba, mode="RGBA")


def _copy_rgba_with_opacity(color_path: Path, opacity_path: Path | None, output: Path) -> None:
    from PIL import Image

    with Image.open(color_path) as color_image:
        rgba = color_image.convert("RGBA")
        if opacity_path is not None and opacity_path.is_file():
            with Image.open(opacity_path) as opacity_image:
                alpha = opacity_image.convert("L")
                if alpha.size != rgba.size:
                    alpha = alpha.resize(rgba.size, Image.Resampling.LANCZOS)
                rgba.putalpha(alpha)
        rgba = _bleed_transparent_rgb(rgba)
        _runtime_safe_image(rgba).save(output)


def _copy_roughness_from_gloss(gloss_path: Path, output: Path) -> None:
    from PIL import Image, ImageOps

    with Image.open(gloss_path) as image:
        roughness = ImageOps.invert(image.convert("L"))
        _runtime_safe_image(roughness).save(output)


def extract_generated_textures(
    document: SpmDocument,
    generated: SpeedTreeGenerated,
    speedtree_root: Path,
    output_dir: Path,
) -> dict[str, str]:
    output_dir.mkdir(parents=True, exist_ok=True)
    for existing in output_dir.iterdir():
        if existing.is_file() and existing.suffix.lower() in {".png", ".jpg", ".jpeg", ".bmp", ".tga", ".dds"}:
            existing.unlink()

    maps = _spm_material_maps(document, speedtree_root)
    written: dict[str, str] = {}
    for material_id, prefix in ((5, "oak_bark"), (13, "oak_cluster_branch"), (14, "oak_leaf")):
        source = maps.get(material_id, {})
        color = source.get("color")
        if color is None:
            continue
        diffuse_out = output_dir / f"{prefix}_diffuse.png"
        _copy_rgba_with_opacity(color, source.get("opacity"), diffuse_out)
        written[f"{prefix}_diffuse"] = str(color)
        normal = source.get("normal")
        if normal is not None:
            _copy_runtime_image(normal, output_dir / f"{prefix}_normal.png", "RGB")
            written[f"{prefix}_normal"] = str(normal)
        gloss = source.get("gloss")
        if gloss is not None:
            _copy_roughness_from_gloss(gloss, output_dir / f"{prefix}_roughness.png")
            written[f"{prefix}_roughness"] = str(gloss)

    # The Modeler-generated atlas is authoritative for geometry assigned to its
    # synthetic material id (23456). Unlike the tiny placeholder material exports,
    # this is the real packed texture atlas produced by SpeedTree.
    atlas_color = generated.export_dir / f"{document.source.stem}_Color.png"
    atlas_normal = generated.export_dir / f"{document.source.stem}_Normal.png"
    # Modeler may choose the requested raw-export basename instead of the SPM stem.
    if not atlas_color.is_file():
        candidates = [p for p in generated.export_dir.glob("*_Color.png") if p.stat().st_size > 4096]
        atlas_color = max(candidates, key=lambda p: p.stat().st_size) if candidates else atlas_color
    if not atlas_normal.is_file():
        candidates = [p for p in generated.export_dir.glob("*_Normal.png") if p.stat().st_size > 4096]
        atlas_normal = max(candidates, key=lambda p: p.stat().st_size) if candidates else atlas_normal
    if atlas_color.is_file():
        from PIL import Image
        with Image.open(atlas_color) as atlas_image:
            atlas_rgba = _bleed_transparent_rgb(atlas_image.convert("RGBA"))
            _runtime_safe_image(atlas_rgba).save(output_dir / "oak_atlas_diffuse.png")
        written["oak_atlas_diffuse"] = str(atlas_color)
    if atlas_normal.is_file():
        _copy_runtime_image(atlas_normal, output_dir / "oak_atlas_normal.png", "RGB")
        written["oak_atlas_normal"] = str(atlas_normal)
    return written


def _convert_collision(collision: dict[str, object], scale: float) -> dict[str, object]:
    p1 = collision["p1"]
    p2 = collision["p2"]
    assert isinstance(p1, list) and isinstance(p2, list)
    return {
        "type": collision["type"],
        "p1": list(_convert_position(float(p1[0]), float(p1[1]), float(p1[2]), scale)),
        "p2": list(_convert_position(float(p2[0]), float(p2[1]), float(p2[2]), scale)),
        "radius": float(collision["radius"]) * scale,
    }


def write_runtime_manifest(
    document: SpmDocument,
    generated: SpeedTreeGenerated,
    output: Path,
    logical_source: str,
    logical_ydd: str,
    material_library_ref: str,
    texture_dictionary_ref: str,
    target_height: float,
    entries: Iterable[Entry],
) -> None:
    entries = list(entries)
    lod_distances = ((0.0, 32.0), (32.0, 72.0), (72.0, 180.0))
    scale = target_height / generated.source_height
    manifest = {
        "schema": RUNTIME_SCHEMA,
        "importer_id": IMPORTER_ID,
        "source_ref": logical_source,
        "source_sha256": sha256(document.source),
        "source_modeler_version": document.modeler_version,
        "generated_format": "SpeedTreeRaw 10",
        "species": document.species,
        "declared_source_triangles": document.declared_triangles,
        "target_height_m": target_height,
        "coordinate_conversion": "SpeedTree_Z_up_to_NorthStar_Y_up:(x,y,z)->(x,z,-y)",
        "geometry_policy": "licensed_speedtree_modeler_generated_exact_mesh",
        "wind": {
            "source_streams": [
                "WindAnchorXYZ", "WindBranchXY", "WindNonBranchXYZ", "WindLeaf2", "GeometryType"
            ],
            "runtime_policy": "northstar_foliage_instanced_wind",
        },
        "collision": [_convert_collision(item, scale) for item in generated.collision],
        "lods": [
            {
                "lod_index": index,
                "min_distance": lod_distances[index][0],
                "max_distance": lod_distances[index][1],
                "drawable_ref": f"{logical_ydd}@{entry.name}",
                "impostor": False,
                "vertices": sum(len(mesh.vertices) for mesh in entry.meshes),
                "indices": sum(len(mesh.indices) for mesh in entry.meshes),
                "parts": len(entry.meshes),
            }
            for index, entry in enumerate(entries)
        ],
        "materials": [
            {"speedtree_material_id": material_id, "material_ref": f"{material_library_ref}@{name}"}
            for material_id, name in MATERIAL_NAMES.items()
        ],
        "texture_dictionary_ref": texture_dictionary_ref,
        "runtime_texture_max_dimension": RUNTIME_TEXTURE_MAX_DIM,
        "billboard_atlas_ref": None,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Import SpeedTree Modeler .spm into NorthStar native foliage")
    parser.add_argument("source", type=Path)
    parser.add_argument("--output-ydd", type=Path)
    parser.add_argument("--entry", default="oak_hero")
    parser.add_argument("--target-height", type=float, default=14.0)
    parser.add_argument("--logical-source", default="Source/foliage/speedtree/oak/Oak_Hero_Forest.spm")
    parser.add_argument("--logical-ydd", default="models/foliage/speedtree/oak_hero.ydd")
    parser.add_argument("--material-library-ref", default="materials/speedtree_oak.nemat")
    parser.add_argument("--texture-dictionary-ref", default="textures/foliage/speedtree/oak_hero.ytd")
    parser.add_argument("--runtime-manifest", type=Path)
    parser.add_argument("--extract-textures", type=Path)
    parser.add_argument("--speedtree-root", type=Path, default=DEFAULT_SPEEDTREE_ROOT)
    parser.add_argument(
        "--lod0-exclude-material",
        type=int,
        action="append",
        default=[],
        help="SpeedTree material ID to omit from near LOD0; may be repeated",
    )
    args = parser.parse_args()

    if not math.isfinite(args.target_height) or args.target_height < 1.0 or args.target_height > 80.0:
        raise SystemExit("--target-height must be finite and between 1 and 80 metres")
    source = args.source.resolve()
    document = read_spm(source)
    speedtree_root = args.speedtree_root.resolve()
    print(f"importer={IMPORTER_ID}")
    print(f"source={source}")
    print(f"modeler_version={document.modeler_version or 'unknown'} species={document.species or 'unknown'}")

    if args.output_ydd is None and args.extract_textures is None:
        raise SystemExit("nothing to do: provide --output-ydd and/or --extract-textures")

    with tempfile.TemporaryDirectory(prefix="northstar-speedtree-") as temp:
        generated = export_generated_speedtree(document, speedtree_root, Path(temp))
        print(
            f"generated=SpeedTreeRaw source_height={generated.source_height:.3f} "
            f"bounds_z=[{generated.source_min_z:.3f},{generated.source_max_z:.3f}] "
            f"collision_objects={len(generated.collision)}"
        )

        if args.extract_textures is not None:
            sources = extract_generated_textures(document, generated, speedtree_root, args.extract_textures.resolve())
            print(f"texture_source_dir={args.extract_textures.resolve()}")
            for key, value in sorted(sources.items()):
                print(f"texture.{key}={value}")

        entries: list[Entry] = []
        if args.output_ydd is not None:
            lod0_excludes = {int(value) for value in args.lod0_exclude_material}
            entries = build_exact_lod_entries(
                generated,
                args.entry,
                args.target_height,
                args.logical_source,
                args.material_library_ref,
                lod0_exclude_materials=lod0_excludes,
            )
            if lod0_excludes:
                print(f"lod0.exclude_materials={','.join(str(v) for v in sorted(lod0_excludes))}")
            write_ydd(entries, args.output_ydd.resolve())
            print(f"output_ydd={args.output_ydd.resolve()}")
            for index, entry in enumerate(entries):
                vertices = sum(len(mesh.vertices) for mesh in entry.meshes)
                indices = sum(len(mesh.indices) for mesh in entry.meshes)
                print(
                    f"lod{index}.entry={entry.name} parts={len(entry.meshes)} "
                    f"vertices={vertices} indices={indices} triangles={indices // 3}"
                )

        if args.runtime_manifest is not None:
            if not entries:
                raise SystemExit("--runtime-manifest requires --output-ydd")
            write_runtime_manifest(
                document,
                generated,
                args.runtime_manifest.resolve(),
                args.logical_source,
                args.logical_ydd,
                args.material_library_ref,
                args.texture_dictionary_ref,
                args.target_height,
                entries,
            )
            print(f"runtime_manifest={args.runtime_manifest.resolve()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
