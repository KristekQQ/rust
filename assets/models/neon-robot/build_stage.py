"""Blender preview-only studio floor, separate from the downloadable character."""
import bpy
from pathlib import Path
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.ops.mesh.primitive_plane_add(size=200,location=(0,0,-.02))
plane=bpy.context.object
plane.name='Dark metallic studio floor'
material=bpy.data.materials.new('Dark satin studio metal')
material.use_nodes=True
p=material.node_tree.nodes.get('Principled BSDF')
p.inputs['Base Color'].default_value=(.004,.003,.007,1)
p.inputs['Metallic'].default_value=.65
p.inputs['Roughness'].default_value=.38
plane.data.materials.append(material)
bpy.ops.export_scene.gltf(filepath=str(Path(__file__).with_name('studio-stage.glb')),
    export_format='GLB',use_selection=True,export_animations=False,export_yup=True)
