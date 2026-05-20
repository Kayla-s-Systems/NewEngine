from __future__ import annotations
import json, math, os, re, struct
from pathlib import Path
from collections import defaultdict
import argparse

parser = argparse.ArgumentParser(description='Pack clouds.zip/clouds DDS profiles into NewEngine .neytd runtime dictionary.')
parser.add_argument('--src', required=True, help='Path to extracted clouds directory containing type subdirectories.')
parser.add_argument('--out', default='assets/textures/fps/clouds_runtime.neytd')
parser.add_argument('--manifest', default='assets/textures/fps/clouds_runtime.manifest.json')
args = parser.parse_args()
SRC = Path(args.src)
OUT = Path(args.out)
MANIFEST = Path(args.manifest)

FORMAT_IDS = {
    'RGBA8_UNORM': 1,
    'RGBA8_SRGB': 2,
    'BC1_RGBA_UNORM': 101,
    'BC1_RGBA_SRGB': 102,
    'BC3_RGBA_UNORM': 103,
    'BC3_RGBA_SRGB': 104,
    'BC5_RG_UNORM': 105,
    'BC7_RGBA_UNORM': 106,
    'BC7_RGBA_SRGB': 107,
}
COLOR_IDS = {'linear': 1, 'srgb': 2}
BLOCK_BYTES = {
    'BC1_RGBA_UNORM': 8,
    'BC1_RGBA_SRGB': 8,
    'BC3_RGBA_UNORM': 16,
    'BC3_RGBA_SRGB': 16,
    'BC5_RG_UNORM': 16,
    'BC7_RGBA_UNORM': 16,
    'BC7_RGBA_SRGB': 16,
}


def normalize_texture_name(name: str) -> str:
    trimmed = name.strip().replace('\\', '/')
    file_name = trimmed.split('/')[-1].strip()
    lower = file_name.lower()
    if '.' not in lower:
        return lower
    stem, ext = lower.rsplit('.', 1)
    if ext in {'png','jpg','jpeg','bmp','tga','webp','dds','ktx','ktx2','neytd','ytd'}:
        return stem
    return lower


def stable_hash64(name: str) -> int:
    h = 0xcbf29ce484222325
    for b in normalize_texture_name(name).encode('utf-8'):
        h ^= b
        h = (h * 0x100000001b3) & 0xffffffffffffffff
    return h


def safe_stem(stem: str) -> str:
    s = re.sub(r'[^a-zA-Z0-9_]+', '_', stem.strip()).strip('_').lower()
    return s or 'texture'


def role_for(stem: str, folder: str) -> str:
    n = stem.lower()
    if folder == 'contrails' or 'contrail' in n:
        return 'contrail_mask'
    if 'detail' in n:
        return 'detail_normal' if ('nrm' in n or n.endswith('_n')) else 'detail_mask'
    if 'marble' in n:
        return 'coverage_mask'
    if n.endswith('_n') or n.endswith('_nrm') or 'normal' in n or n.endswith('trialn'):
        return 'normal'
    if n.endswith('_ap') or n.endswith('trialap') or '_ap_' in n:
        return 'albedo_alpha'
    return 'density_mask'


def parse_dds(path: Path) -> dict:
    b = path.read_bytes()
    if len(b) < 128 or b[:4] != b'DDS ':
        raise ValueError(f'{path}: not a DDS file')
    size = struct.unpack_from('<I', b, 4)[0]
    if size != 124:
        raise ValueError(f'{path}: unsupported DDS header size {size}')
    height = struct.unpack_from('<I', b, 12)[0]
    width = struct.unpack_from('<I', b, 16)[0]
    mip_count = struct.unpack_from('<I', b, 28)[0] or 1
    fourcc = b[84:88]
    data_offset = 128
    if fourcc == b'DX10':
        if len(b) < 148:
            raise ValueError(f'{path}: truncated DX10 DDS')
        dxgi = struct.unpack_from('<I', b, 128)[0]
        data_offset = 148
        # Minimal mapping for common BCn DXGI ids.
        if dxgi in (71, 72):
            fmt = 'BC1_RGBA_UNORM'
        elif dxgi in (77, 78):
            fmt = 'BC3_RGBA_UNORM'
        elif dxgi == 83:
            fmt = 'BC5_RG_UNORM'
        elif dxgi in (98, 99):
            fmt = 'BC7_RGBA_UNORM'
        else:
            raise ValueError(f'{path}: unsupported DX10 format id {dxgi}')
    elif fourcc == b'DXT1':
        fmt = 'BC1_RGBA_UNORM'
    elif fourcc == b'DXT5':
        fmt = 'BC3_RGBA_UNORM'
    elif fourcc in (b'ATI2', b'BC5U'):
        fmt = 'BC5_RG_UNORM'
    else:
        raise ValueError(f'{path}: unsupported DDS FourCC {fourcc!r}')

    block_bytes = BLOCK_BYTES[fmt]
    offset = data_offset
    mips = []
    for level in range(mip_count):
        w = max(1, width >> level)
        h = max(1, height >> level)
        byte_len = max(1, (w + 3) // 4) * max(1, (h + 3) // 4) * block_bytes
        if offset + byte_len > len(b):
            raise ValueError(f'{path}: truncated mip level={level} expected={byte_len} at={offset} len={len(b)}')
        mips.append({'level': level, 'width': w, 'height': h, 'bytes': b[offset:offset+byte_len]})
        offset += byte_len
    return {'width': width, 'height': height, 'mip_count': mip_count, 'format': fmt, 'mips': mips, 'fourcc': fourcc.decode('ascii', 'ignore')}


def align16(buf: bytearray):
    while len(buf) % 16:
        buf.append(0)


def write_u16(buf: bytearray, off: int, v: int):
    struct.pack_into('<H', buf, off, v)

def write_u32(buf: bytearray, off: int, v: int):
    struct.pack_into('<I', buf, off, v)

def write_u64(buf: bytearray, off: int, v: int):
    struct.pack_into('<Q', buf, off, v)

files = sorted(SRC.glob('*/*.dds'))
if not files:
    raise SystemExit('no DDS files found')

entries = []
profiles = defaultdict(list)
seen_names = set()
for path in files:
    folder = path.parent.name
    stem = path.stem
    name = normalize_texture_name(f'cloud_{safe_stem(folder)}__{safe_stem(stem)}')
    if name in seen_names:
        raise ValueError(f'duplicate generated entry name: {name}')
    seen_names.add(name)
    dds = parse_dds(path)
    role = role_for(stem, folder)
    entries.append({
        'name': name,
        'folder': folder,
        'source': str(path.relative_to(SRC)),
        'role': role,
        'width': dds['width'],
        'height': dds['height'],
        'format': dds['format'],
        'color_space': 'linear',
        'mips': dds['mips'],
        'fourcc': dds['fourcc'],
    })

entries.sort(key=lambda e: e['name'])

# Raw data region with exact mip block bytes. Deduplicate identical mip payloads but keep per-entry metadata.
data = bytearray()
dedup: dict[bytes, tuple[int,int]] = {}
for e in entries:
    start = None
    end = None
    mip_meta = []
    for mip in e['mips']:
        payload = mip['bytes']
        if payload in dedup:
            off, length = dedup[payload]
        else:
            align16(data)
            off = len(data)
            data.extend(payload)
            length = len(payload)
            dedup[payload] = (off, length)
        if start is None or off < start:
            start = off
        end = max(end or 0, off + length)
        mip_meta.append({'byte_offset': off, 'byte_len': length, 'width': mip['width'], 'height': mip['height'], 'level': mip['level']})
    e['byte_offset'] = start or 0
    e['byte_len'] = (end or 0) - (start or 0)
    e['mip_meta'] = mip_meta
    e['name_hash'] = stable_hash64(e['name'])

# Directory
entry_count = len(entries)
mip_count = sum(len(e['mip_meta']) for e in entries)
DIR_HEADER = 40
ENTRY_LEN = 64
MIP_LEN = 32
entries_offset = DIR_HEADER
mips_offset = entries_offset + entry_count * ENTRY_LEN
names_offset = mips_offset + mip_count * MIP_LEN
names = bytearray()
dir_buf = bytearray(names_offset)
dir_buf[0:4] = b'NTDX'
write_u16(dir_buf, 4, 1)
write_u16(dir_buf, 6, ENTRY_LEN)
write_u16(dir_buf, 8, MIP_LEN)
write_u16(dir_buf, 10, 0)
write_u32(dir_buf, 12, entry_count)
write_u32(dir_buf, 16, mip_count)
write_u32(dir_buf, 20, entries_offset)
write_u32(dir_buf, 24, mips_offset)
write_u32(dir_buf, 28, names_offset)

mip_cursor = 0
for i, e in enumerate(entries):
    name_offset = len(names)
    name_bytes = e['name'].encode('utf-8')
    names.extend(name_bytes)
    off = entries_offset + i * ENTRY_LEN
    write_u64(dir_buf, off + 0, e['name_hash'])
    write_u64(dir_buf, off + 8, e['byte_offset'])
    write_u64(dir_buf, off + 16, e['byte_len'])
    write_u32(dir_buf, off + 24, name_offset)
    write_u16(dir_buf, off + 28, len(name_bytes))
    write_u16(dir_buf, off + 30, FORMAT_IDS[e['format']])
    write_u32(dir_buf, off + 32, e['width'])
    write_u32(dir_buf, off + 36, e['height'])
    write_u32(dir_buf, off + 40, mip_cursor)
    write_u32(dir_buf, off + 44, len(e['mip_meta']))
    write_u16(dir_buf, off + 48, COLOR_IDS[e['color_space']])
    for mip in e['mip_meta']:
        moff = mips_offset + mip_cursor * MIP_LEN
        write_u64(dir_buf, moff + 0, mip['byte_offset'])
        write_u64(dir_buf, moff + 8, mip['byte_len'])
        write_u32(dir_buf, moff + 16, mip['width'])
        write_u32(dir_buf, moff + 20, mip['height'])
        write_u16(dir_buf, moff + 24, mip['level'])
        mip_cursor += 1

write_u32(dir_buf, 32, len(names))
dir_buf.extend(names)

# Header + file
header = bytearray(64)
header[0:4] = b'NETD'
write_u16(header, 4, 2)
write_u16(header, 6, 0)           # raw runtime data
write_u32(header, 8, 64)
write_u32(header, 12, entry_count)
write_u64(header, 16, 64)
write_u64(header, 24, len(dir_buf))
write_u64(header, 32, 64 + len(dir_buf))
write_u64(header, 40, len(data))
write_u64(header, 48, 0)
OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_bytes(header + dir_buf + data)

manifest = {
    'dictionary': 'textures/fps/clouds_runtime.neytd',
    'entry_count': entry_count,
    'mip_count': mip_count,
    'payload_bytes': len(data),
    'directory_bytes': len(dir_buf),
    'source': 'clouds.zip/clouds',
    'profiles': {},
    'entries': [],
}
for e in entries:
    item = {
        'name': e['name'],
        'selector': f"textures/fps/clouds_runtime.neytd@{e['name']}",
        'profile': e['folder'],
        'role': e['role'],
        'source': e['source'],
        'format': e['format'],
        'color_space': e['color_space'],
        'width': e['width'],
        'height': e['height'],
        'mip_count': len(e['mip_meta']),
        'name_hash': str(e['name_hash']),
    }
    manifest['entries'].append(item)
    manifest['profiles'].setdefault(e['folder'], []).append({k: item[k] for k in ('name','selector','role','format','width','height','mip_count')})
MANIFEST.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
print(f'wrote {OUT} entries={entry_count} mips={mip_count} file_bytes={OUT.stat().st_size} data_bytes={len(data)}')
print(f'wrote {MANIFEST}')
