#!/usr/bin/env python3
"""Patch a NorthStar YMT metadata XML skeleton from an authoritative OpenFormats .skel.

The OpenFormats bone traversal order is preserved verbatim because skinned YDD vertices
address joints by this dense index. Names, tags, parent relationships and bind-pose local
transforms therefore come from the same source as the mesh weights.
"""

from __future__ import annotations

import argparse
import re
from dataclasses import dataclass
from pathlib import Path
from xml.sax.saxutils import quoteattr


@dataclass
class Joint:
    index: int
    indent: int
    name: str
    tag: int
    parent_index: int
    rotation: tuple[float, float, float, float] = (0.0, 0.0, 0.0, 1.0)
    translation: tuple[float, float, float] = (0.0, 0.0, 0.0)
    scale: tuple[float, float, float] = (1.0, 1.0, 1.0)
    flags: tuple[str, ...] = ()


BONE_RE = re.compile(r"^(?P<indent>\s*)Bone\s+(?P<name>\S+)\s+(?P<tag>\d+)\s*$")
FLOAT4_RE = re.compile(r"^\s*RotationQuaternion\s+(.+)$")
FLOAT3_RE = re.compile(r"^\s*(LocalOffset|Scale)\s+(.+)$")
FLAGS_RE = re.compile(r"^\s*Flags(?:\s+(.*))?$")

_FLAG_NAMES = {
    "ROT_X": "RotX",
    "ROT_Y": "RotY",
    "ROT_Z": "RotZ",
    "TRANS_X": "TransX",
    "TRANS_Y": "TransY",
    "TRANS_Z": "TransZ",
}


def _floats(raw: str, count: int) -> tuple[float, ...]:
    values = tuple(float(value) for value in raw.split())
    if len(values) != count:
        raise ValueError(f"expected {count} floats, got {len(values)}: {raw!r}")
    return values


def parse_openformats_skeleton(path: Path) -> list[Joint]:
    joints: list[Joint] = []
    stack: list[Joint] = []
    current: Joint | None = None

    for line in path.read_text("utf-8", errors="strict").splitlines():
        bone_match = BONE_RE.match(line)
        if bone_match:
            indent = len(bone_match.group("indent").replace("    ", "\t"))
            while stack and stack[-1].indent >= indent:
                stack.pop()
            parent_index = stack[-1].index if stack else -1
            current = Joint(
                index=len(joints),
                indent=indent,
                name=bone_match.group("name"),
                tag=int(bone_match.group("tag")),
                parent_index=parent_index,
            )
            joints.append(current)
            stack.append(current)
            continue

        if current is None:
            continue
        match = FLOAT4_RE.match(line)
        if match:
            current.rotation = _floats(match.group(1), 4)  # type: ignore[assignment]
            continue
        match = FLOAT3_RE.match(line)
        if match:
            values = _floats(match.group(2), 3)
            if match.group(1) == "LocalOffset":
                current.translation = values  # type: ignore[assignment]
            else:
                current.scale = values  # type: ignore[assignment]
            continue
        match = FLAGS_RE.match(line)
        if match:
            raw_flags = (match.group(1) or "").split()
            current.flags = tuple(
                _FLAG_NAMES.get(flag, flag.title().replace("_", "")) for flag in raw_flags
            )

    if not joints:
        raise ValueError(f"{path}: no Bone records found")
    expected = re.search(r"\bNumBones\s+(\d+)", path.read_text("utf-8", errors="strict"))
    if expected and int(expected.group(1)) != len(joints):
        raise ValueError(f"{path}: NumBones={expected.group(1)} parsed={len(joints)}")
    return joints


def _f(value: float) -> str:
    if abs(value) < 5.0e-12:
        value = 0.0
    return format(value, ".9g")


def render_joint(joint: Joint, joints: list[Joint]) -> str:
    parent_name = joints[joint.parent_index].name if joint.parent_index >= 0 else None
    attrs = [
        f'index="{joint.index}"',
        f'tag="{joint.tag}"',
        f'name={quoteattr(joint.name)}',
        f'parent_index="{joint.parent_index}"',
        f'tx="{_f(joint.translation[0])}"',
        f'ty="{_f(joint.translation[1])}"',
        f'tz="{_f(joint.translation[2])}"',
        f'qx="{_f(joint.rotation[0])}"',
        f'qy="{_f(joint.rotation[1])}"',
        f'qz="{_f(joint.rotation[2])}"',
        f'qw="{_f(joint.rotation[3])}"',
        f'sx="{_f(joint.scale[0])}"',
        f'sy="{_f(joint.scale[1])}"',
        f'sz="{_f(joint.scale[2])}"',
    ]
    if parent_name is not None:
        attrs.append(f'parent={quoteattr(parent_name)}')
    if joint.flags:
        attrs.append(f'flags={quoteattr(",".join(joint.flags))}')
    return "      <Joint " + " ".join(attrs) + " />"


def authoring_source_label(path: Path) -> str:
    """Return a portable authoring path and never serialize a machine-local prefix."""
    parts = path.parts
    for index in range(len(parts) - 1, -1, -1):
        if parts[index].lower() == "source":
            return "/".join(parts[index:])
    return path.name


def patch_ymt_xml(source_skel: Path, target_xml: Path) -> None:
    joints = parse_openformats_skeleton(source_skel)
    source_label = authoring_source_label(source_skel)
    text = target_xml.read_text("utf-8", errors="strict")
    skeleton_open = re.search(r"<Skeleton\b[^>]*>", text)
    if not skeleton_open:
        raise ValueError(f"{target_xml}: missing Skeleton element")
    anchors_pos = text.find("      <Anchors", skeleton_open.end())
    if anchors_pos < 0:
        raise ValueError(f"{target_xml}: missing Anchors element")
    first_joint = text.find("      <Joint ", skeleton_open.end(), anchors_pos)
    if first_joint < 0:
        raise ValueError(f"{target_xml}: missing Joint records")

    opening = skeleton_open.group(0)
    opening = re.sub(
        r'source_format="[^"]*"',
        'source_format="rage.openformats.skel.v165"',
        opening,
    )
    opening = re.sub(
        r'source="[^"]*"',
        f'source={quoteattr(source_label)}',
        opening,
    )
    opening = re.sub(r'joint_count="\d+"', f'joint_count="{len(joints)}"', opening)
    text = text[: skeleton_open.start()] + opening + text[skeleton_open.end() :]

    # Positions may have shifted if the opening tag length changed; locate again.
    skeleton_open = re.search(r"<Skeleton\b[^>]*>", text)
    assert skeleton_open is not None
    anchors_pos = text.find("      <Anchors", skeleton_open.end())
    first_joint = text.find("      <Joint ", skeleton_open.end(), anchors_pos)
    joint_block = "\n".join(render_joint(joint, joints) for joint in joints) + "\n"
    text = text[:first_joint] + joint_block + text[anchors_pos:]
    target_xml.write_text(text, "utf-8", newline="\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("source_skel", type=Path)
    parser.add_argument("target_ymt_xml", type=Path)
    args = parser.parse_args()
    patch_ymt_xml(args.source_skel.resolve(), args.target_ymt_xml.resolve())
    print(f"[YMT] patched {args.target_ymt_xml} from {args.source_skel}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
