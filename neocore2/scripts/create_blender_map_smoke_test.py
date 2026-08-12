import bpy, math, sys
from pathlib import Path

argv = sys.argv[sys.argv.index('--') + 1:] if '--' in sys.argv else []
out = Path(argv[argv.index('--output') + 1]).resolve()
out.parent.mkdir(parents=True, exist_ok=True)

bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)


def add_cube(name, mesh_name, location, scale=(1,1,1), collision=False, tags=''):
    bpy.ops.mesh.primitive_cube_add(location=location, scale=scale)
    obj = bpy.context.active_object
    obj.name = name
    obj.data.name = mesh_name
    if collision:
        obj['ns_collision'] = True
    if tags:
        obj['ns_tags'] = tags
    return obj

# 96x96 m floor intentionally crosses 64 m map-cell boundaries.
ground = add_cube('Ground', 'ground_mesh', (0,0,-0.5), (48,48,0.5), True, 'ground,collision,smoke_test')

# One shared Blender mesh, five placements across several cells.
crate = add_cube('Crate_A', 'crate_mesh', (0,0,1), tags='prop,crate,smoke_test')
for name, loc, yaw in [
    ('Crate_B', (12,8,1), 24),
    ('Crate_C', (72,6,1), 36),
    ('Crate_D', (-72,-10,1), 48),
    ('Crate_E', (6,-70,1), 60),
]:
    dup = crate.copy()
    dup.data = crate.data
    dup.name = name
    dup.location = loc
    dup.rotation_euler[2] = math.radians(yaw)
    bpy.context.collection.objects.link(dup)

# Same source mesh, but forced unique due to object-specific modifier.
unique = crate.copy()
unique.data = crate.data
unique.name = 'Crate_Beveled_Unique'
unique.location = (22,-16,1)
unique.scale = (1.4,0.8,1.8)
unique['ns_unique_mesh'] = True
unique['ns_tags'] = 'prop,crate,unique_modifier,smoke_test'
bpy.context.collection.objects.link(unique)
bev = unique.modifiers.new('SmokeTestBevel', 'BEVEL')
bev.width = 0.18
bev.segments = 3

bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=2, depth=8, location=(20,20,4))
tower = bpy.context.active_object
tower.name = 'Tower'
tower.data.name = 'tower_mesh'
tower['ns_collision'] = True
tower['ns_tags'] = 'landmark,tower,collision,smoke_test'

ramp = add_cube('Ramp', 'ramp_mesh', (-18,14,1), (5,2,0.5), True, 'ramp,collision,smoke_test')
ramp.rotation_euler[1] = math.radians(-12)

bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, radius=2, location=(0,0,6))
ignored = bpy.context.active_object
ignored.name = 'Ignored_DebugSphere'
ignored['ns_map_ignore'] = True

bpy.context.scene.name = 'NorthStarMapImportSmokeTest'
bpy.context.scene.unit_settings.system = 'METRIC'
bpy.context.scene.unit_settings.scale_length = 1.0
bpy.context.scene['northstar_map_id'] = 'blender_map_smoke_test'
bpy.context.scene['northstar_cell_size'] = 64.0

text = bpy.data.texts.new('NORTHSTAR_MAP_IMPORT_README')
text.write('North Star Blender -> YMAP v2 smoke test. Linked crates must share one runtime mesh; ignored sphere must not be exported.')

bpy.ops.wm.save_as_mainfile(filepath=str(out), check_existing=False)
print(f'NORTHSTAR_SMOKE_BLEND_OK path={out}')
print(f'mesh_objects={len([o for o in bpy.context.scene.objects if o.type == "MESH"])}')
