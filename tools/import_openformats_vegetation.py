#!/usr/bin/env python3
"""Import OpenFormats ODD/ODR text meshes into NorthStar native NEF8/YDD v2.

The importer is deliberately dependency-free. It preserves source positions,
normals, UV0 and triangle indices, converts OpenFormats Z-up coordinates to
NorthStar Y-up coordinates, and stores every ODR drawable as an addressable YDD
entry. Source LOD variants therefore remain separate entries and are not rendered
simultaneously as mesh parts.
"""
from __future__ import annotations

import argparse
import math
import re
import struct
import zlib
from dataclasses import dataclass
from pathlib import Path

YDD_SCHEMA_VERSION = 2
NEF8_WIRE_VERSION = 2
NEF8_CONTENT_KIND_YDD = 2
NEF8_FLAG_BODY_DEFLATE = 0x0001
BODY_HEADER_LEN = 40
ENTRY_RECORD_LEN = 80
MESH_HEADER_LEN = 40
VERTEX_STRIDE = 32
NONE_STRING = 0xFFFFFFFF


@dataclass
class Vertex:
    position: tuple[float, float, float]
    normal: tuple[float, float, float]
    uv: tuple[float, float]


@dataclass
class Mesh:
    name: str
    material_ref: str | None
    vertices: list[Vertex]
    indices: list[int]


@dataclass
class Entry:
    name: str
    source_path: str
    meshes: list[Mesh]


def _u32(value: int) -> bytes:
    return struct.pack('<I', value)


def _u64(value: int) -> bytes:
    return struct.pack('<Q', value)


def _f32(value: float) -> bytes:
    if not math.isfinite(value):
        raise ValueError(f'non-finite f32: {value}')
    return struct.pack('<f', value)


def _vec3(value: tuple[float, float, float]) -> bytes:
    return b''.join(_f32(component) for component in value)


def _joaat(text: str) -> int:
    value = 0
    for byte in text.lower().encode('ascii'):
        value = (value + byte) & 0xFFFFFFFF
        value = (value + (value << 10)) & 0xFFFFFFFF
        value ^= value >> 6
    value = (value + (value << 3)) & 0xFFFFFFFF
    value ^= value >> 11
    value = (value + (value << 15)) & 0xFFFFFFFF
    return value & 0xFFFFFFFF


HASHED_GRASS_NAMES = {
    _joaat(f'grass{index:02d}{suffix}'): f'grass{index:02d}{suffix}'
    for index in range(16, 32)
    for suffix in ('', '_lod1', '_lod2')
}


def canonical_entry_name(source_name: str) -> str:
    if re.fullmatch(r'0x[0-9A-Fa-f]{8}', source_name):
        hashed = int(source_name, 16)
        return HASHED_GRASS_NAMES.get(hashed, source_name)
    return source_name


def material_ref_for_entry(entry_name: str) -> str | None:
    if re.fullmatch(r'grass(?:0[0-9]|[12][0-9]|3[01])(?:_lod[12])?', entry_name, re.I):
        return f'materials/vegetation.nemat@{entry_name.lower()}'
    return None


def _fnv1a64(text: str) -> int:
    value = 0xCBF29CE484222325
    for byte in text.encode('utf-8'):
        value ^= byte
        value = (value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def _axis_xyz_to_xz_minus_y(v: tuple[float, float, float]) -> tuple[float, float, float]:
    return (v[0], v[2], -v[1])


def _parse_vec(text: str, count: int) -> tuple[float, ...]:
    values = tuple(float(token) for token in text.strip().split())
    if len(values) < count:
        raise ValueError(f'expected {count} components, got {len(values)} in {text!r}')
    return values[:count]


def parse_mesh(path: Path, entry_name: str, material_ref: str | None) -> list[Mesh]:
    lines = path.read_text(encoding='utf-8', errors='strict').replace('\r', '').splitlines()
    meshes: list[Mesh] = []
    cursor = 0
    geometry_index = 0
    while cursor < len(lines):
        if lines[cursor].strip() != 'Geometry':
            cursor += 1
            continue
        cursor += 1
        while cursor < len(lines) and not lines[cursor].strip().startswith('Indices '):
            cursor += 1
        if cursor >= len(lines):
            raise ValueError(f'{path}: Geometry has no Indices block')
        index_count = int(lines[cursor].strip().split()[1])
        cursor += 1
        indices: list[int] = []
        while cursor < len(lines) and len(indices) < index_count:
            stripped = lines[cursor].strip()
            if stripped.startswith('Vertices '):
                break
            if stripped and not stripped.startswith(('{', '}')):
                indices.extend(int(token) for token in stripped.split())
            cursor += 1
        if len(indices) != index_count:
            raise ValueError(f'{path}: expected {index_count} indices, got {len(indices)}')
        while cursor < len(lines) and not lines[cursor].strip().startswith('Vertices '):
            cursor += 1
        if cursor >= len(lines):
            raise ValueError(f'{path}: Geometry has no Vertices block')
        vertex_count = int(lines[cursor].strip().split()[1])
        cursor += 1
        vertices: list[Vertex] = []
        while cursor < len(lines) and len(vertices) < vertex_count:
            stripped = lines[cursor].strip()
            cursor += 1
            if not stripped or '/' not in stripped:
                continue
            groups = [group.strip() for group in stripped.split('/')]
            if len(groups) < 3:
                raise ValueError(f'{path}: unsupported vertex record {stripped!r}')
            source_pos = _parse_vec(groups[0], 3)
            source_nrm = _parse_vec(groups[1], 3)
            uv = _parse_vec(groups[-1], 2)
            pos = _axis_xyz_to_xz_minus_y(source_pos)  # type: ignore[arg-type]
            nrm = _axis_xyz_to_xz_minus_y(source_nrm)  # type: ignore[arg-type]
            vertices.append(Vertex(pos, nrm, (uv[0], uv[1])))
        if len(vertices) != vertex_count:
            raise ValueError(f'{path}: expected {vertex_count} vertices, got {len(vertices)}')
        if index_count % 3:
            raise ValueError(f'{path}: non-triangular index count {index_count}')
        if any(index < 0 or index >= vertex_count for index in indices):
            raise ValueError(f'{path}: index outside vertex range')
        mesh_name = f'leaf_{entry_name}' if geometry_index == 0 else f'leaf_{entry_name}_{geometry_index}'
        meshes.append(Mesh(mesh_name, material_ref, vertices, indices))
        geometry_index += 1
    if not meshes:
        raise ValueError(f'{path}: no Geometry blocks found')
    return meshes


def parse_odd(odd_path: Path) -> list[Entry]:
    source_root = odd_path.parent
    text = odd_path.read_text(encoding='utf-8', errors='strict').replace('\r', '')
    refs = re.findall(r'^\s*([^\s]+\.odr)\s*$', text, re.MULTILINE | re.IGNORECASE)
    if not refs:
        raise ValueError(f'{odd_path}: no ODR entries found')
    entries: list[Entry] = []
    seen: set[str] = set()
    for ref in refs:
        normalized_ref = ref.replace('\\', '/')
        odr_path = source_root / Path(normalized_ref)
        if not odr_path.is_file():
            raise FileNotFoundError(f'{odd_path}: missing ODR {normalized_ref}')
        source_name = odr_path.stem
        name = canonical_entry_name(source_name)
        key = name.casefold()
        if key in seen:
            raise ValueError(f'{odd_path}: duplicate entry name {name!r}')
        seen.add(key)
        odr = odr_path.read_text(encoding='utf-8', errors='strict').replace('\r', '')
        shader_match = re.search(r'Shaders\s*\{\s*([^\s\{]+)\s*\{', odr, re.DOTALL)
        shader = shader_match.group(1) if shader_match else ''
        if shader.casefold() != 'grass.sps':
            raise ValueError(f'{odr_path}: expected grass.sps vegetation shader, got {shader!r}')
        mesh_refs = re.findall(r'^\s*([^\s]+\.mesh)\s+\d+\s*$', odr, re.MULTILINE | re.IGNORECASE)
        if not mesh_refs:
            raise ValueError(f'{odr_path}: no mesh refs found')
        meshes: list[Mesh] = []
        for mesh_ref in mesh_refs:
            mesh_path = odr_path.parent / Path(mesh_ref.replace('\\', '/'))
            if not mesh_path.is_file():
                raise FileNotFoundError(f'{odr_path}: missing mesh {mesh_ref}')
            meshes.extend(parse_mesh(mesh_path, name, material_ref_for_entry(name)))
        entries.append(Entry(name=name, source_path=normalized_ref, meshes=meshes))
    return entries


def bounds_for_vertices(vertices: list[Vertex]) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
    if not vertices:
        raise ValueError('cannot compute empty bounds')
    mins = [float('inf')] * 3
    maxs = [float('-inf')] * 3
    for vertex in vertices:
        for axis, value in enumerate(vertex.position):
            mins[axis] = min(mins[axis], value)
            maxs[axis] = max(maxs[axis], value)
    return (tuple(mins), tuple(maxs))  # type: ignore[return-value]


def bounds_for_entry(entry: Entry) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
    return bounds_for_vertices([vertex for mesh in entry.meshes for vertex in mesh.vertices])


def encode_ydd_body(entries: list[Entry]) -> bytes:
    strings = bytearray()
    offsets: dict[str, int] = {}

    def string_offset(value: str) -> int:
        if value not in offsets:
            offsets[value] = len(strings)
            strings.extend(value.encode('utf-8'))
            strings.append(0)
        return offsets[value]

    for entry in entries:
        string_offset(entry.name)
        string_offset(entry.source_path)
        for mesh in entry.meshes:
            string_offset(mesh.name)
            if mesh.material_ref is not None:
                string_offset(mesh.material_ref)

    table_offset = BODY_HEADER_LEN
    string_table_offset = table_offset + len(entries) * ENTRY_RECORD_LEN
    payload_floor = string_table_offset + len(strings)

    payloads: list[bytes] = []
    for entry in entries:
        payload = bytearray()
        payload += _u32(len(entry.meshes))
        payload += _u32(0)
        for mesh in entry.meshes:
            bmin, bmax = bounds_for_vertices(mesh.vertices)
            payload += _u32(string_offset(mesh.name))
            payload += _u32(string_offset(mesh.material_ref) if mesh.material_ref is not None else NONE_STRING)
            payload += _u32(len(mesh.vertices))
            payload += _u32(len(mesh.indices))
            payload += _vec3(bmin)
            payload += _vec3(bmax)
            for vertex in mesh.vertices:
                payload += _vec3(vertex.position)
                payload += _vec3(vertex.normal)
                payload += _f32(vertex.uv[0])
                payload += _f32(vertex.uv[1])
            for index in mesh.indices:
                payload += _u32(index)
        payloads.append(bytes(payload))

    # string_offset() above is intentionally complete before offsets are frozen.
    if payload_floor != string_table_offset + len(strings):
        raise AssertionError('string table mutated after payload layout calculation')

    out = bytearray()
    out += _u32(YDD_SCHEMA_VERSION)
    out += _u32(len(entries))
    out += _u64(table_offset)
    out += _u64(string_table_offset)
    out += _u64(len(strings))
    out += _u64(payload_floor)

    payload_cursor = payload_floor
    for entry, payload in zip(entries, payloads):
        bmin, bmax = bounds_for_entry(entry)
        vertex_count = sum(len(mesh.vertices) for mesh in entry.meshes)
        index_count = sum(len(mesh.indices) for mesh in entry.meshes)
        out += _u64(_fnv1a64(entry.name))
        out += _u32(string_offset(entry.name))
        out += _u32(string_offset(entry.source_path))
        out += _u32(len(entry.meshes))
        out += _u32(vertex_count)
        out += _u32(index_count)
        out += _u32(0)
        out += _u32(NONE_STRING)
        out += _vec3(bmin)
        out += _vec3(bmax)
        out += _u32(0)
        out += _u64(payload_cursor)
        out += _u64(len(payload))
        payload_cursor += len(payload)

    if len(out) != string_table_offset:
        raise AssertionError(f'entry table size mismatch {len(out)} != {string_table_offset}')
    out += strings
    for payload in payloads:
        out += payload
    if len(out) != payload_cursor:
        raise AssertionError(f'body length mismatch {len(out)} != {payload_cursor}')
    return bytes(out)


def raw_deflate(data: bytes) -> bytes:
    compressor = zlib.compressobj(level=9, method=zlib.DEFLATED, wbits=-15)
    return compressor.compress(data) + compressor.flush()


def encode_nef8_ydd(body: bytes, entry_count: int) -> bytes:
    stored = raw_deflate(body)
    header = bytearray(32)
    header[0:4] = b'NEF8'
    header[4] = NEF8_WIRE_VERSION
    header[5] = 5  # 32-byte header: stored + uncompressed lengths
    struct.pack_into('<H', header, 6, NEF8_CONTENT_KIND_YDD)
    struct.pack_into('<H', header, 8, NEF8_FLAG_BODY_DEFLATE)
    struct.pack_into('<H', header, 10, YDD_SCHEMA_VERSION)
    struct.pack_into('<I', header, 12, entry_count)
    struct.pack_into('<Q', header, 16, len(stored))
    struct.pack_into('<Q', header, 24, len(body))
    return bytes(header) + stored


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument('odd', type=Path, help='OpenFormats .odd drawable dictionary')
    parser.add_argument('output', type=Path, help='NorthStar native .ydd output')
    args = parser.parse_args()
    entries = parse_odd(args.odd.resolve())
    body = encode_ydd_body(entries)
    encoded = encode_nef8_ydd(body, len(entries))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(encoded)
    vertices = sum(len(mesh.vertices) for entry in entries for mesh in entry.meshes)
    indices = sum(len(mesh.indices) for entry in entries for mesh in entry.meshes)
    lod_variants = sum(1 for entry in entries if re.search(r'_lod\d+$', entry.name, re.I))
    print(f'output={args.output}')
    print(f'entries={len(entries)} lod_variants={lod_variants} vertices={vertices} indices={indices} triangles={indices // 3}')
    print(f'body_bytes={len(body)} stored_bytes={len(encoded)-32} file_bytes={len(encoded)}')
    print('axis=OpenFormats_Z_up_to_NorthStar_Y_up:(x,y,z)->(x,z,-y)')
    print('material_policy=grass.sps -> materials/vegetation.nemat@entry; alpha-cutout/two-sided lives in NEMAT')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
