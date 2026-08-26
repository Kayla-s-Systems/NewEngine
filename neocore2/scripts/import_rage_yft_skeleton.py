#!/usr/bin/env python3
"""Project a RAGE/OpenFormats YFT XML skeleton into a NewEngine YMT NEF8 asset.

The script intentionally reuses the workspace-owned `tools/maintenance/nef8_wire.py`
writer instead of duplicating the NEF8 wire envelope in importer code.
"""
from __future__ import annotations

import argparse
import sys
import xml.etree.ElementTree as ET
import zlib
from pathlib import Path

YMT_TYPE_ID = 10


def _workspace_root() -> Path:
    here = Path(__file__).resolve()
    for parent in (here.parent, *here.parents):
        helper = parent / "tools" / "maintenance" / "nef8_wire.py"
        if helper.is_file():
            return parent
    raise SystemExit("NorthStar workspace root with tools/maintenance/nef8_wire.py not found")


def _value(node: ET.Element, child: str, default: str = "") -> str:
    item = node.find(child)
    return item.attrib.get("value", default) if item is not None else default


def _inflate_ymt(path: Path):
    workspace = _workspace_root()
    sys.path.insert(0, str(workspace / "tools" / "maintenance"))
    from nef8_wire import parse_header

    packed = path.read_bytes()
    header = parse_header(packed)
    if header.type_id != YMT_TYPE_ID:
        raise SystemExit(f"{path}: expected YMT type_id={YMT_TYPE_ID}, got {header.type_id}")
    body = zlib.decompress(
        packed[header.body_offset : header.body_offset + header.body_len],
        -15,
    )
    metadata = packed[header.metadata_offset : header.metadata_offset + header.metadata_len]
    return packed, header, metadata, ET.fromstring(body)


def _inject_skeleton(root: ET.Element, yft_path: Path, source_ref: str, entry_name: str) -> int:
    entry = next(
        (
            node
            for node in root.iter()
            if node.tag.lower() == "entry" and node.attrib.get("name") == entry_name
        ),
        None,
    )
    if entry is None:
        raise SystemExit(f"YMT metadata entry '{entry_name}' not found")
    for child in list(entry):
        if child.tag.lower() == "skeleton":
            entry.remove(child)

    yft_root = ET.parse(yft_path).getroot()
    bones = yft_root.find("./Drawable/Skeleton/Bones")
    if bones is None:
        raise SystemExit(f"{yft_path}: Drawable/Skeleton/Bones not found")
    bone_items = list(bones)
    if not bone_items:
        raise SystemExit(f"{yft_path}: skeleton contains no bones")

    names = {
        int(_value(bone, "Index")): (bone.findtext("Name") or "").strip()
        for bone in bone_items
    }
    skeleton = ET.SubElement(
        entry,
        "Skeleton",
        {
            "schema": "newengine.skeleton.bind_pose.v1",
            "source_format": "rage.yft.xml",
            "source": source_ref,
            "joint_count": str(len(bone_items)),
        },
    )
    for bone in bone_items:
        index = int(_value(bone, "Index"))
        parent_index = int(_value(bone, "ParentIndex", "-1"))
        translation = bone.find("Translation")
        rotation = bone.find("Rotation")
        scale = bone.find("Scale")
        if translation is None or rotation is None or scale is None:
            raise SystemExit(f"bone index={index}: incomplete bind transform")
        attrs = {
            "index": str(index),
            "tag": _value(bone, "Tag", "0"),
            "name": (bone.findtext("Name") or "").strip(),
            "parent_index": str(parent_index),
            "tx": translation.attrib.get("x", "0"),
            "ty": translation.attrib.get("y", "0"),
            "tz": translation.attrib.get("z", "0"),
            "qx": rotation.attrib.get("x", "0"),
            "qy": rotation.attrib.get("y", "0"),
            "qz": rotation.attrib.get("z", "0"),
            "qw": rotation.attrib.get("w", "1"),
            "sx": scale.attrib.get("x", "1"),
            "sy": scale.attrib.get("y", "1"),
            "sz": scale.attrib.get("z", "1"),
        }
        if parent_index >= 0:
            attrs["parent"] = names[parent_index]
        flags = (bone.findtext("Flags") or "").strip()
        if flags:
            attrs["flags"] = ",".join(part.strip() for part in flags.split(",") if part.strip())
        ET.SubElement(skeleton, "Joint", attrs)

    ET.SubElement(
        skeleton,
        "Anchors",
        {
            "root": "SKEL_ROOT",
            "hips": "SKEL_Pelvis",
            "head": "SKEL_Head",
            "left_hand": "SKEL_L_Hand",
            "right_hand": "SKEL_R_Hand",
            "left_foot": "SKEL_L_Foot",
            "right_foot": "SKEL_R_Foot",
            "eye": "FACIAL_L_eyeball",
        },
    )
    return len(bone_items)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--yft-xml", required=True, type=Path)
    parser.add_argument("--base-ymt", required=True, type=Path)
    parser.add_argument("--authoring-out", required=True, type=Path)
    parser.add_argument("--runtime-out", required=True, type=Path)
    parser.add_argument("--entry", default="csb_abigail")
    parser.add_argument("--source-ref", required=True)
    args = parser.parse_args()

    _, header, metadata, root = _inflate_ymt(args.base_ymt)
    count = _inject_skeleton(root, args.yft_xml, args.source_ref, args.entry)
    ET.indent(root, space="  ")
    body = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        + ET.tostring(root, encoding="unicode")
        + "\n"
    ).encode("utf-8")
    args.authoring_out.parent.mkdir(parents=True, exist_ok=True)
    args.authoring_out.write_bytes(body)

    workspace = _workspace_root()
    sys.path.insert(0, str(workspace / "tools" / "maintenance"))
    from nef8_wire import blake3_reference_digest, encode_deflated

    packed = encode_deflated(
        body,
        type_id=header.type_id,
        content_schema_version=header.content_schema_version,
        entry_count=header.entry_count,
        header_metadata=metadata,
        min_size_class=max(6, header.size_class),
        body_hash=blake3_reference_digest(body),
        stable_file_id=header.stable_file_id or None,
        import_settings_hash=header.import_settings_hash or None,
    )
    args.runtime_out.parent.mkdir(parents=True, exist_ok=True)
    args.runtime_out.write_bytes(packed)
    print(
        f"RAGE skeleton imported joints={count} authoring='{args.authoring_out}' "
        f"runtime='{args.runtime_out}' bytes={len(packed)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
