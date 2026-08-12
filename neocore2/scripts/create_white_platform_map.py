#!/usr/bin/env python3
from __future__ import annotations
import argparse, sys
from pathlib import Path
import bpy

def parse_args():
    argv=sys.argv
    argv=argv[argv.index('--')+1:] if '--' in argv else []
    ap=argparse.ArgumentParser(); ap.add_argument('--output',required=True,type=Path); return ap.parse_args(argv)

def main():
    o=parse_args(); out=o.output.expanduser().resolve(); out.parent.mkdir(parents=True,exist_ok=True)
    bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
    bpy.ops.mesh.primitive_cube_add(location=(0.0,0.0,-0.5), scale=(40.0,40.0,0.5))
    platform=bpy.context.active_object
    platform.name='WhitePlatform'; platform.data.name='white_platform_mesh'
    platform['ns_collision']=True
    platform['ns_material']='materials/white_platform.nemat@white_platform'
    platform['ns_tags']='ground,platform,white,collision'
    preview=bpy.data.materials.new('WhitePlatformPreview'); preview.diffuse_color=(1.0,1.0,1.0,1.0); preview.use_nodes=True
    bsdf=preview.node_tree.nodes.get('Principled BSDF') if preview.node_tree else None
    if bsdf is not None:
        bsdf.inputs['Base Color'].default_value=(1.0,1.0,1.0,1.0)
        bsdf.inputs['Metallic'].default_value=0.0
        bsdf.inputs['Roughness'].default_value=0.72
    platform.data.materials.append(preview)

    # Dynamic physics crates: one shared mesh/YDD, several independently simulated placements.
    bpy.ops.mesh.primitive_cube_add(location=(0.0, 0.0, 1.25), scale=(0.65, 0.65, 0.65))
    crate=bpy.context.active_object
    crate.name='PhysicsCrate_01'; crate.data.name='physics_crate_mesh'
    crate['ns_collision']=True
    crate['ns_material']='materials/physics_crate.nemat@crate'
    crate['ns_apply_mode']='dynamic_physics'
    crate['ns_tags']='physics,dynamic,crate'
    placements=[
        ('PhysicsCrate_02',(0.15,0.10,3.2),(0.0,0.0,0.18),(0.72,0.72,0.72)),
        ('PhysicsCrate_03',(-0.25,-0.10,5.3),(0.12,0.05,-0.24),(0.62,0.62,0.62)),
        ('PhysicsCrate_04',(3.2,0.4,4.6),(0.18,-0.10,0.45),(0.85,0.60,0.70)),
        ('PhysicsCrate_05',(-3.0,-0.6,6.2),(-0.20,0.12,-0.38),(0.58,0.82,0.66)),
        ('PhysicsCrate_06',(1.7,-2.8,7.5),(0.25,0.18,0.30),(0.68,0.68,0.68)),
        ('PhysicsCrate_07',(-1.8,2.7,8.8),(-0.18,0.22,-0.28),(0.75,0.55,0.80)),
    ]
    for name,loc,rot,scale in placements:
        obj=crate.copy(); obj.data=crate.data; obj.name=name; obj.location=loc; obj.rotation_euler=rot; obj.scale=scale
        bpy.context.collection.objects.link(obj)

    bpy.context.scene.name='NorthStarWhitePlatform'
    bpy.context.scene.unit_settings.system='METRIC'; bpy.context.scene.unit_settings.scale_length=1.0
    bpy.context.scene['northstar_map_id']='white_platform'; bpy.context.scene['northstar_cell_size']=64.0
    txt=bpy.data.texts.new('NORTHSTAR_WHITE_PLATFORM_README'); txt.write('North Star minimal white platform map. One 80x80x1m static mesh, collision enabled, matte white PBR material. Runtime material: materials/white_platform.nemat@white_platform')
    bpy.ops.wm.save_as_mainfile(filepath=str(out), check_existing=False)
    print(f'NORTHSTAR_WHITE_PLATFORM_BLEND_OK path={out}'); print('mesh_objects=8 platform=1 dynamic_crates=7 size=80x80x1 top_z=0')
    return 0

if __name__=='__main__': raise SystemExit(main())
