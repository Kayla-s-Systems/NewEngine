#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import re
import struct
from dataclasses import dataclass
from pathlib import Path

GEOMETRY_RE = re.compile(
    r"Geometry\s*\{\s*ShaderIndex\s+(?P<shader>\d+).*?"
    r"VertexDeclaration\s+(?P<decl>\S+)\s*Indices\s+(?P<ic>\d+)\s*"
    r"\{(?P<indices>.*?)\}\s*Vertices\s+(?P<vc>\d+)\s*"
    r"\{(?P<vertices>.*?)\}\s*\}", re.S)
AABB_RE = re.compile(
    r"Aabb\s*\{\s*Min\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s*"
    r"Max\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)", re.S)
ODR_AABB_RE = re.compile(
    r"AABBMin\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+).*?"
    r"AABBMax\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)", re.S)
ODD_REF_RE = re.compile(r"^\s*[^\\/]+[\\/](\S+\.odr)\s*$", re.I | re.M)
SLOTS = {"head":"head", "hair":"hair", "uppr":"upper", "lowr":"lower",
         "hand":"hands", "accs":"accessories", "decl":"decals"}

# OpenFormats GTA V ped mesh vertices are authored around a shared character-space
# origin one metre below the runtime skeleton/world origin. This is NOT an ODR
# per-component translation. ODR LodGroup AABBs are culling metadata and can differ
# from raw mesh AABBs (notably lowr/accs), so deriving component transforms from their
# centres tears multipart characters apart.
PED_BIND_ORIGIN_Z = 1.0

@dataclass
class Geometry:
    slot: str
    positions: list[tuple[float,float,float]]
    normals: list[tuple[float,float,float]]
    uvs: list[tuple[float,float]]
    joints: list[tuple[int,int,int,int]]
    weights: list[tuple[float,float,float,float]]
    indices: list[int]

@dataclass
class Component:
    name: str
    odr_min: tuple[float,float,float]
    odr_max: tuple[float,float,float]
    raw_min: tuple[float,float,float]
    raw_max: tuple[float,float,float]
    geometries: list[Geometry]

def parse_vertex(line: str):
    groups = [g.strip().split() for g in line.strip().split("/")]
    if (len(groups) < 7 or len(groups[0]) < 3 or len(groups[1]) < 4
            or len(groups[2]) < 4 or len(groups[3]) < 3 or len(groups[6]) < 2):
        raise ValueError(f"unsupported OpenFormats skinned vertex layout: {line[:160]!r}")
    position = tuple(map(float, groups[0][:3]))
    raw_weights = tuple(map(float, groups[1][:4]))
    joints = tuple(map(int, groups[2][:4]))
    if min(joints) < 0 or max(joints) > 65535:
        raise ValueError(f"joint index outside GLTF UNSIGNED_SHORT range: {joints}")
    weight_sum = sum(max(0.0, value) for value in raw_weights)
    if not math.isfinite(weight_sum) or weight_sum <= 1.0e-8:
        raise ValueError(f"invalid skin weights: {raw_weights}")
    weights = tuple(max(0.0, value) / weight_sum for value in raw_weights)
    normal = tuple(map(float, groups[3][:3]))
    uv0 = tuple(map(float, groups[6][:2]))
    return position, normal, uv0, joints, weights

def slot_for(name: str) -> str:
    prefix = name.split("_", 1)[0].lower()
    if prefix not in SLOTS:
        raise ValueError(f"unsupported ped component '{name}'")
    return SLOTS[prefix]

def parse_component(root: Path, odr_name: str) -> Component:
    name = Path(odr_name).stem
    odr = root / odr_name
    mesh = root / name / f"{name}_high.mesh"
    ot = odr.read_text("utf-8", errors="replace")
    mt = mesh.read_text("utf-8", errors="replace")
    om = ODR_AABB_RE.search(ot)
    if not om:
        raise ValueError(f"ODR has no AABB: {odr}")
    ov = tuple(map(float, om.groups()))
    aabbs = [tuple(map(float, m.groups())) for m in AABB_RE.finditer(mt)]
    if not aabbs:
        raise ValueError(f"mesh has no AABB: {mesh}")
    rmin = tuple(min(a[i] for a in aabbs) for i in range(3))
    rmax = tuple(max(a[i+3] for a in aabbs) for i in range(3))
    slot = slot_for(name)
    geoms = []
    for gi, m in enumerate(GEOMETRY_RE.finditer(mt)):
        indices = [int(v) for v in m.group("indices").split()]
        lines = [line.strip() for line in m.group("vertices").splitlines() if line.strip()]
        if len(indices) != int(m.group("ic")) or len(lines) != int(m.group("vc")):
            raise ValueError(f"count mismatch {mesh} geometry={gi}")
        if len(indices) % 3:
            raise ValueError(f"non-triangle indices {mesh} geometry={gi}")
        positions, normals, uvs, joints, weights = [], [], [], [], []
        for line in lines:
            p, n, uv, vertex_joints, vertex_weights = parse_vertex(line)
            positions.append(p); normals.append(n); uvs.append(uv)
            joints.append(vertex_joints); weights.append(vertex_weights)
        if indices and max(indices) >= len(positions):
            raise ValueError(f"index out of range {mesh} geometry={gi}")
        geoms.append(Geometry(slot, positions, normals, uvs, joints, weights, indices))
    if not geoms:
        raise ValueError(f"mesh has no geometry: {mesh}")
    return Component(name, ov[:3], ov[3:], rmin, rmax, geoms)

def norm(v):
    length = math.sqrt(sum(x*x for x in v))
    return (0.0,1.0,0.0) if length < 1e-8 else tuple(x/length for x in v)

def transform(components: list[Component], target_height: float):
    # Every multipart ped component shares the same bind-space origin. Apply that
    # once, then derive global bounds from the actual vertices. Never use ODR AABB
    # centres as transforms: lowr and accs prove those are not placement matrices.
    source_min = [math.inf, math.inf, math.inf]
    source_max = [-math.inf, -math.inf, -math.inf]
    for c in components:
        for g in c.geometries:
            for x, y, z in g.positions:
                z += PED_BIND_ORIGIN_Z
                source_min[0] = min(source_min[0], x)
                source_min[1] = min(source_min[1], y)
                source_min[2] = min(source_min[2], z)
                source_max[0] = max(source_max[0], x)
                source_max[1] = max(source_max[1], y)
                source_max[2] = max(source_max[2], z)

    height = source_max[2] - source_min[2]
    if not math.isfinite(height) or height <= 1.0e-6:
        raise ValueError(f"invalid ped bind-space height: {height}")
    scale = target_height / height

    # Preserve skeleton origin in X/Y. Only move the lowest authored point onto
    # NewEngine ground Y=0 while converting GTA Z-up -> NewEngine Y-up.
    for c in components:
        for g in c.geometries:
            outp = []
            for x, y, z in g.positions:
                z += PED_BIND_ORIGIN_Z
                outp.append((x * scale, (z - source_min[2]) * scale, -y * scale))
            g.positions = outp
            g.normals = [norm((nx,nz,-ny)) for nx,ny,nz in g.normals]

    # Column-major affine transform that maps raw RAGE mesh/skeleton coordinates
    # into the baked model space used above. Retaining this transform is required for
    # palette conjugation: P_model = M * P_source * inverse(M).
    translate_y = (PED_BIND_ORIGIN_Z - source_min[2]) * scale
    source_to_model = (
        scale, 0.0, 0.0, 0.0,
        0.0, 0.0, -scale, 0.0,
        0.0, scale, 0.0, 0.0,
        0.0, translate_y, 0.0, 1.0,
    )
    return tuple(source_min), tuple(source_max), scale, source_to_model

def pad4(buf: bytearray, value=0):
    while len(buf) % 4: buf.append(value)

def build_glb(components: list[Component], output: Path, source_to_model: tuple[float, ...]):
    binary = bytearray(); views=[]; accessors=[]; meshes=[]
    total_v = total_i = skinned_v = 0
    max_joint_index = 0
    def add_data(values, fmt, target, component_type, type_name):
        pad4(binary); offset=len(binary); count=0
        for value in values:
            vals = value if isinstance(value, tuple) else (value,)
            binary.extend(struct.pack(fmt, *vals)); count += 1
        view=len(views); views.append({"buffer":0,"byteOffset":offset,"byteLength":len(binary)-offset,"target":target})
        acc=len(accessors); accessors.append({"bufferView":view,"byteOffset":0,"componentType":component_type,"count":count,"type":type_name})
        return acc
    for c in components:
        for g in c.geometries:
            pa=add_data(g.positions,"<fff",34962,5126,"VEC3")
            na=add_data(g.normals,"<fff",34962,5126,"VEC3")
            ua=add_data(g.uvs,"<ff",34962,5126,"VEC2")
            ja=add_data(g.joints,"<HHHH",34962,5123,"VEC4")
            wa=add_data(g.weights,"<ffff",34962,5126,"VEC4")
            ia=add_data(g.indices,"<I",34963,5125,"SCALAR")
            meshes.append({"name":g.slot,"primitives":[{"attributes":{
                "POSITION":pa,"NORMAL":na,"TEXCOORD_0":ua,"JOINTS_0":ja,"WEIGHTS_0":wa
            },"indices":ia,"mode":4}]})
            total_v += len(g.positions); total_i += len(g.indices); skinned_v += len(g.joints)
            if g.joints:
                max_joint_index = max(max_joint_index, max(max(j) for j in g.joints))
    pad4(binary)
    doc={"asset":{"version":"2.0","generator":"NorthStar OpenFormats Ped Importer"},
         "buffers":[{"byteLength":len(binary)}],"bufferViews":views,"accessors":accessors,"meshes":meshes,
         "extras":{"northstar":{"skin_source_space":"rage_z_up","skin_source_to_model":list(source_to_model)}}}
    jb=json.dumps(doc,separators=(",",":"),ensure_ascii=False).encode("utf-8")
    while len(jb)%4: jb += b" "
    bb=bytes(binary)
    total=12+8+len(jb)+8+len(bb)
    out=bytearray(b"glTF")+struct.pack("<II",2,total)+struct.pack("<II",len(jb),0x4E4F534A)+jb+struct.pack("<II",len(bb),0x004E4942)+bb
    output.parent.mkdir(parents=True,exist_ok=True); output.write_bytes(out)
    return {
        "bytes":len(out),"mesh_parts":len(meshes),"vertices":total_v,"indices":total_i,
        "skinned_vertices":skinned_v,"max_joint_index":max_joint_index
    }

def main():
    ap=argparse.ArgumentParser(description="OpenFormats RAGE ped bind-pose -> GLB")
    ap.add_argument("--source-root",type=Path,required=True)
    ap.add_argument("--odd",type=Path)
    ap.add_argument("--output",type=Path,required=True)
    ap.add_argument("--target-height",type=float,default=1.78)
    a=ap.parse_args(); root=a.source_root.resolve()
    candidates=[a.odd, root/"csb_abigail.odd", root.parent/"csb_abigail.odd"]
    odd=next((p.resolve() for p in candidates if p and p.is_file()),None)
    if not odd: raise SystemExit("ODD not found")
    refs=ODD_REF_RE.findall(odd.read_text("utf-8",errors="replace"))
    if not refs: raise SystemExit(f"no ODR references in {odd}")
    components=[parse_component(root, Path(ref).name) for ref in refs]
    wmin,wmax,scale,source_to_model=transform(components,a.target_height)
    result=build_glb(components,a.output.resolve(),source_to_model)
    result.update({"output":str(a.output.resolve()),"components":[c.name for c in components],"source_bounds":[wmin,wmax],"bind_origin_z":PED_BIND_ORIGIN_Z,"scale":scale,"source_to_model":source_to_model,"target_height":a.target_height})
    print(json.dumps(result,indent=2))
if __name__=="__main__": main()
