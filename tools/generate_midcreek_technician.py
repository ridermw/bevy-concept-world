"""Generate the first Midcreek male-technician GLB with Blender.

The vertical slice deliberately reuses the qualified Quaternius armature and
animations. The visible character is assembled from low-detail, rigid modules
parented to that armature so silhouette, scale, palette, and equipment can be
validated in Bevy before committing to final topology and skinning.
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector


PALETTE = {
    "skin": "#C98F6A",
    "hair": "#2B2320",
    "shirt": "#55707F",
    "denim": "#4A6485",
    "vest": "#C8D94A",
    "trim": "#E8763A",
    "silver": "#D6DBE0",
    "hard_hat": "#2C6FB8",
    "boots": "#3A3128",
    "belt": "#302A25",
    "tools": "#C6782D",
    "defenders": "#30363B",
    "eyes": "#23282D",
}


def parse_args() -> argparse.Namespace:
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args(args)


def rgba(hex_color: str) -> tuple[float, float, float, float]:
    value = hex_color.removeprefix("#")
    return tuple(int(value[index : index + 2], 16) / 255 for index in (0, 2, 4)) + (
        1.0,
    )


def material(name: str, color: str) -> bpy.types.Material:
    result = bpy.data.materials.new(name)
    result.diffuse_color = rgba(color)
    result.metallic = 0.0
    result.roughness = 0.9
    return result


def finish_mesh(
    obj: bpy.types.Object,
    name: str,
    mat: bpy.types.Material,
    armature: bpy.types.Object,
    bone: str,
    bevel: float = 0.0,
) -> bpy.types.Object:
    obj.name = name
    obj.data.name = f"{name}Mesh"
    obj.data.materials.append(mat)
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
    if bevel > 0.0:
        modifier = obj.modifiers.new("Small hard bevel", "BEVEL")
        modifier.width = bevel
        modifier.segments = 1
        bpy.ops.object.modifier_apply(modifier=modifier.name)
    # These solid-color modules do not use textures. Removing generated UVs
    # also avoids run-to-run float noise from Blender's bevel UV interpolation.
    while obj.data.uv_layers:
        obj.data.uv_layers.remove(obj.data.uv_layers[0])
    for polygon in obj.data.polygons:
        polygon.use_smooth = False
    world = obj.matrix_world.copy()
    obj.parent = armature
    obj.parent_type = "BONE"
    obj.parent_bone = bone
    obj.matrix_world = world
    obj.select_set(False)
    return obj


def add_box(
    name: str,
    location: tuple[float, float, float],
    dimensions: tuple[float, float, float],
    mat: bpy.types.Material,
    armature: bpy.types.Object,
    bone: str,
    *,
    bevel: float = 0.015,
) -> bpy.types.Object:
    bpy.ops.mesh.primitive_cube_add(location=location)
    obj = bpy.context.object
    obj.scale = tuple(value / 2 for value in dimensions)
    return finish_mesh(obj, name, mat, armature, bone, bevel)


def add_sphere(
    name: str,
    location: tuple[float, float, float],
    dimensions: tuple[float, float, float],
    mat: bpy.types.Material,
    armature: bpy.types.Object,
    bone: str,
) -> bpy.types.Object:
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1.0, location=location)
    obj = bpy.context.object
    obj.scale = tuple(value / 2 for value in dimensions)
    return finish_mesh(obj, name, mat, armature, bone)


def add_cylinder(
    name: str,
    location: tuple[float, float, float],
    dimensions: tuple[float, float, float],
    mat: bpy.types.Material,
    armature: bpy.types.Object,
    bone: str,
    *,
    rotation: tuple[float, float, float] = (0.0, 0.0, 0.0),
) -> bpy.types.Object:
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=12,
        radius=0.5,
        depth=1.0,
        location=location,
        rotation=rotation,
    )
    obj = bpy.context.object
    obj.scale = dimensions
    return finish_mesh(obj, name, mat, armature, bone, 0.008)


def bone_points(
    armature: bpy.types.Object, bone_name: str
) -> tuple[Vector, Vector]:
    bone = armature.data.bones[bone_name]
    return armature.matrix_world @ bone.head_local, armature.matrix_world @ bone.tail_local


def add_limb(
    name: str,
    armature: bpy.types.Object,
    bone_name: str,
    radius: float,
    mat: bpy.types.Material,
    *,
    shorten: float = 0.90,
) -> bpy.types.Object:
    head, tail = bone_points(armature, bone_name)
    direction = tail - head
    midpoint = head + direction * 0.5
    bpy.ops.mesh.primitive_cylinder_add(
        vertices=10,
        radius=radius,
        depth=direction.length * shorten,
        location=midpoint,
    )
    obj = bpy.context.object
    obj.rotation_mode = "QUATERNION"
    obj.rotation_quaternion = direction.to_track_quat("Z", "Y")
    return finish_mesh(obj, name, mat, armature, bone_name, 0.008)


def make_hidden_contract_mesh(mannequin: bpy.types.Object) -> None:
    mannequin.name = "HiddenRigContractMesh"
    mannequin.scale = (0.001, 0.001, 0.001)
    mannequin.location.z = -10.0


def build_character(armature: bpy.types.Object) -> None:
    mats = {name: material(f"Midcreek_{name}", color) for name, color in PALETTE.items()}

    # Core proportions: boots begin at ground level and the hard-hat crown
    # reaches 1.73 m, matching the approved character scale sheet.
    add_box("JeansPelvis", (0.0, 0.0, 0.92), (0.36, 0.24, 0.24), mats["denim"], armature, "pelvis")
    add_box("WorkShirtTorso", (0.0, 0.0, 1.30), (0.43, 0.23, 0.43), mats["shirt"], armature, "spine_02", bevel=0.035)
    add_box("HighVisibilityVest", (0.0, -0.005, 1.33), (0.46, 0.245, 0.34), mats["vest"], armature, "spine_02", bevel=0.025)
    add_box("VestSilverBand", (0.0, -0.012, 1.27), (0.475, 0.255, 0.065), mats["silver"], armature, "spine_02", bevel=0.008)
    for side in (-1, 1):
        add_box(
            f"VestShoulderBand_{side:+d}",
            (side * 0.135, -0.132, 1.41),
            (0.065, 0.018, 0.20),
            mats["silver"],
            armature,
            "spine_03",
            bevel=0.004,
        )
        add_box(
            f"VestOrangeTrim_{side:+d}",
            (side * 0.205, -0.135, 1.34),
            (0.018, 0.016, 0.31),
            mats["trim"],
            armature,
            "spine_02",
            bevel=0.003,
        )

    add_cylinder("Neck", (0.0, 0.0, 1.54), (0.07, 0.07, 0.10), mats["skin"], armature, "neck_01")
    add_sphere("Head", (0.0, -0.01, 1.625), (0.19, 0.18, 0.20), mats["skin"], armature, "Head")
    add_sphere("Hair", (0.0, 0.015, 1.665), (0.195, 0.17, 0.105), mats["hair"], armature, "Head")
    add_sphere("HardHatShell", (0.0, 0.0, 1.692), (0.235, 0.205, 0.076), mats["hard_hat"], armature, "Head")
    add_box("HardHatBrim", (0.0, -0.075, 1.675), (0.245, 0.115, 0.022), mats["hard_hat"], armature, "Head", bevel=0.008)
    add_box("Nose", (0.0, -0.105, 1.625), (0.035, 0.035, 0.045), mats["skin"], armature, "Head", bevel=0.006)
    for side in (-1, 1):
        add_sphere(
            f"Eye_{side:+d}",
            (side * 0.038, -0.101, 1.647),
            (0.018, 0.012, 0.018),
            mats["eyes"],
            armature,
            "Head",
        )
        add_cylinder(
            f"EarDefender_{side:+d}",
            (side * 0.115, 0.0, 1.665),
            (0.045, 0.06, 0.045),
            mats["defenders"],
            armature,
            "Head",
            rotation=(0.0, math.pi / 2, 0.0),
        )
        add_sphere(
            f"CordedEarPlug_{side:+d}",
            (side * 0.082, -0.082, 1.615),
            (0.022, 0.018, 0.022),
            mats["trim"],
            armature,
            "Head",
        )

    add_box("ToolBelt", (0.0, 0.0, 1.00), (0.40, 0.255, 0.065), mats["belt"], armature, "pelvis", bevel=0.012)
    for side in (-1, 1):
        add_box(
            f"ToolPouch_{side:+d}",
            (side * 0.205, 0.005, 0.91),
            (0.10, 0.17, 0.19),
            mats["tools"],
            armature,
            "pelvis",
            bevel=0.018,
        )

    for side in ("l", "r"):
        add_limb(f"LongSleeveUpperArm_{side}", armature, f"upperarm_{side}", 0.072, mats["shirt"])
        add_limb(f"LongSleeveForearm_{side}", armature, f"lowerarm_{side}", 0.062, mats["shirt"])
        hand_head, hand_tail = bone_points(armature, f"hand_{side}")
        hand_location = hand_head.lerp(hand_tail, 0.65)
        add_sphere(
            f"Hand_{side}",
            tuple(hand_location),
            (0.115, 0.10, 0.12),
            mats["skin"],
            armature,
            f"hand_{side}",
        )

        add_limb(f"RoomyDenimThigh_{side}", armature, f"thigh_{side}", 0.105, mats["denim"], shorten=0.96)
        add_limb(f"RoomyDenimCalf_{side}", armature, f"calf_{side}", 0.09, mats["denim"], shorten=0.94)
        foot_head, _ = bone_points(armature, f"foot_{side}")
        add_box(
            f"WorkBoot_{side}",
            (foot_head.x, -0.075, 0.07),
            (0.18, 0.31, 0.14),
            mats["boots"],
            armature,
            f"foot_{side}",
            bevel=0.025,
        )


def main() -> None:
    args = parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)

    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(args.source))

    armature = next(obj for obj in bpy.context.scene.objects if obj.type == "ARMATURE")
    mannequin = next(obj for obj in bpy.context.scene.objects if obj.type == "MESH")
    armature.name = "MidcreekTechnicianRig"
    make_hidden_contract_mesh(mannequin)
    build_character(armature)

    bpy.context.scene.name = "Scene"
    bpy.ops.export_scene.gltf(
        filepath=str(args.output),
        export_format="GLB",
        export_animations=True,
        export_animation_mode="NLA_TRACKS",
        export_skins=True,
        export_morph=False,
        export_cameras=False,
        export_lights=False,
        export_yup=True,
        check_existing=False,
    )
    print(f"generated {args.output}")


if __name__ == "__main__":
    main()
