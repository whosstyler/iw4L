use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anim_iw4::dobj_surface_hidden;
use asset_game::{WeaponAnimSlot, WeaponAnimations};
use bevy::prelude::{Mat4, Quat, Vec3};
use serde_json::{Value, json};
use weapon_iw4::WEAPON_ANIM_SLOTS;
use xmodel_runtime::{AnimClip, AnimInstance, DObj};

use super::{GltfBuilder, INCHES_TO_METERS, basis_conversion, safe_name, write_png};
use crate::PreparedMatch;

const EYE_CAMERA_YFOV_RAD: f32 = 0.688;
const EYE_CAMERA_ASPECT: f32 = 16.0 / 9.0;
const EYE_CAMERA_ZNEAR_M: f32 = 0.01;
const EYE_CAMERA_ZFAR_M: f32 = 100.0;
const COMPONENT_F32: u32 = 5126;
const COMPONENT_U16: u32 = 5123;
const TARGET_ARRAY_BUFFER: u32 = 34962;

#[derive(Debug)]
pub struct ViewmodelExportSummary {
    pub scene: PathBuf,
    pub bones: usize,
    pub primitives: u64,
    pub triangles: u64,
    pub hidden_surfaces: u64,
    pub animations: Vec<String>,
    pub refusals: BTreeMap<&'static str, u64>,
}

impl ViewmodelExportSummary {
    pub fn report_line(&self) -> String {
        format!(
            "viewmodel glTF: bones={} primitives={} triangles={} hidden_surfaces={} animations=[{}] refusals={:?}",
            self.bones,
            self.primitives,
            self.triangles,
            self.hidden_surfaces,
            self.animations.join(", "),
            self.refusals,
        )
    }
}

pub fn export_prepared_viewmodel_gltf(
    artifacts: &Path,
    weapon: &str,
    prepared: &PreparedMatch,
) -> Result<ViewmodelExportSummary, String> {
    let weapons = &prepared.weapons;
    let id = match weapons.resolve_index(weapon) {
        Ok(Some(id)) => id,
        Ok(None) => return Err("empty weapon name".into()),
        Err(_) => {
            let mut names = (1..=weapons.len() as u32)
                .map(|index| weapons.name_of(index))
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>();
            names.sort_unstable();
            names.dedup();
            return Err(format!(
                "unknown weapon `{weapon}`; this map loads: {}",
                names.join(", ")
            ));
        }
    };
    let assembly = weapons
        .fpv_assemblies_of(id, false)
        .map(|sides| sides.bare.clone())
        .ok_or_else(|| {
            let reason = weapons
                .fpv_mount_error_of(id)
                .map(|error| format!("{error:?}"))
                .unwrap_or_else(|| "no first-person assembly".into());
            format!("weapon `{weapon}` has no first-person viewmodel: {reason}")
        })?;
    let dobj = &assembly.dobj;
    let animations = WeaponAnimations::from_registry(weapons, id, &prepared.xanims);
    let clips = (0..WEAPON_ANIM_SLOTS)
        .filter_map(WeaponAnimSlot::from_index)
        .filter_map(|slot| Some((slot, animations.clip(slot)?.clone())))
        .collect::<Vec<_>>();

    let rest = match animations.clip(WeaponAnimSlot::Idle) {
        Some(idle) => sample_pose(dobj, idle, 0.0),
        None => dobj.compose(&dobj.bind_locals(), Mat4::IDENTITY),
    };
    let rest_gltf = rest.iter().map(|world| to_gltf(*world)).collect::<Vec<_>>();
    let bake = dobj.skin_matrices(&rest);

    let mut out = GltfBuilder::default();
    let joints = add_bone_nodes(&mut out, dobj, &rest_gltf);
    let camera_node = out.nodes.len();
    out.nodes.push(json!({
        "name": "eye_camera",
        "camera": 0,
        "rotation": [0.0, -core::f32::consts::FRAC_1_SQRT_2, 0.0, core::f32::consts::FRAC_1_SQRT_2],
    }));
    push_child(&mut out.nodes[assembly.view_bone], camera_node);

    let inverse_binds = rest_gltf
        .iter()
        .flat_map(|world| world.inverse().to_cols_array())
        .collect::<Vec<_>>();
    let inverse_bind_accessor = data_accessor(&mut out, &inverse_binds, rest_gltf.len(), "MAT4");

    let mut hidden_surfaces = 0u64;
    let mut mesh_nodes = Vec::new();
    for part in &assembly.parts {
        let entry = prepared
            .fpv_meshes
            .get_at(part.model.order())
            .ok_or_else(|| format!("assembly part {:?} missing from the catalog", part.role))?;
        let mut primitives = Vec::new();
        for surface in entry.skel.surfaces_for_lod(0) {
            let hidden = part.hide.is_some_and(|hide| {
                entry
                    .skel
                    .surface_part_bits
                    .get(surface)
                    .is_some_and(|bits| dobj_surface_hidden(bits, &hide, 0))
            });
            if hidden {
                hidden_surfaces += 1;
                continue;
            }
            let Some(material) = surface_material(&mut out, prepared, entry, surface) else {
                continue;
            };
            match skinned_primitive(&mut out, entry, surface, part.bone_base, &bake, material) {
                Ok(primitive) => primitives.push(primitive),
                Err(reason) => out.refuse(reason),
            }
        }
        if primitives.is_empty() {
            continue;
        }
        let mesh = out.meshes.len();
        out.meshes.push(json!({
            "name": entry.skel.name,
            "primitives": primitives,
        }));
        mesh_nodes.push(out.nodes.len());
        out.nodes.push(json!({
            "name": entry.skel.name,
            "mesh": mesh,
            "skin": 0,
            "extras": {"iw4l": {"role": format!("{:?}", part.role)}},
        }));
    }
    if out.primitives == 0 {
        return Err(format!(
            "no viewmodel surface survived export; refusals={:?}",
            out.refusals
        ));
    }

    let mut exported = Vec::new();
    let mut gltf_animations = Vec::new();
    for (slot, clip) in &clips {
        let name = slot_name(*slot);
        gltf_animations.push(animation_json(&mut out, dobj, clip, &name));
        exported.push(name);
    }

    let root = out.nodes.len();
    let root_children = joints
        .roots
        .iter()
        .copied()
        .chain(mesh_nodes.iter().copied())
        .collect::<Vec<_>>();
    out.nodes.push(json!({
        "name": format!("viewmodel_{}", safe_name(weapon)),
        "children": root_children,
    }));

    let document = json!({
        "asset": {"version": "2.0", "generator": "iw4l viewmodel exporter"},
        "scene": 0,
        "scenes": [{"name": weapon, "nodes": [root]}],
        "nodes": out.nodes,
        "meshes": out.meshes,
        "skins": [{
            "name": "viewmodel",
            "joints": (0..rest_gltf.len()).collect::<Vec<_>>(),
            "inverseBindMatrices": inverse_bind_accessor,
        }],
        "animations": gltf_animations,
        "cameras": [{
            "type": "perspective",
            "perspective": {
                "yfov": EYE_CAMERA_YFOV_RAD,
                "aspectRatio": EYE_CAMERA_ASPECT,
                "znear": EYE_CAMERA_ZNEAR_M,
                "zfar": EYE_CAMERA_ZFAR_M,
            },
        }],
        "materials": out.materials,
        "textures": out.textures,
        "images": out.images,
        "samplers": out.samplers,
        "accessors": out.accessors,
        "bufferViews": out.buffer_views,
        "buffers": [{"uri": "scene.bin", "byteLength": out.bin.len()}],
        "extras": {"iw4l": {
            "weapon": weapons.name_of(id),
            "view_bone": dobj.bones.get(assembly.view_bone).map(|bone| bone.name.as_str()),
            "rest_pose": if animations.clip(WeaponAnimSlot::Idle).is_some() { "idle frame 0" } else { "bind" },
        }},
    });

    let export_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before unix epoch: {error}"))?
        .as_millis();
    let output = artifacts
        .join("exports")
        .join("viewmodel")
        .join(safe_name(weapon))
        .join(format!("vm-{export_id}"));
    fs::create_dir_all(output.join("textures"))
        .map_err(|error| format!("create {}: {error}", output.display()))?;
    fs::write(output.join("scene.bin"), &out.bin)
        .map_err(|error| format!("write scene.bin: {error}"))?;
    for image in &out.pending_images {
        write_png(&output.join("textures").join(&image.file_name), image)?;
    }
    let gltf = serde_json::to_vec_pretty(&document)
        .map_err(|error| format!("serialize scene.gltf: {error}"))?;
    let scene = output.join("scene.gltf");
    fs::write(&scene, gltf).map_err(|error| format!("write {}: {error}", scene.display()))?;

    Ok(ViewmodelExportSummary {
        scene,
        bones: rest_gltf.len(),
        primitives: out.primitives,
        triangles: out.triangles,
        hidden_surfaces,
        animations: exported,
        refusals: out.refusals,
    })
}

struct BoneNodes {
    roots: Vec<usize>,
}

fn add_bone_nodes(out: &mut GltfBuilder, dobj: &DObj, rest_gltf: &[Mat4]) -> BoneNodes {
    let base = out.nodes.len();
    let mut roots = Vec::new();
    for (index, bone) in dobj.bones.iter().enumerate() {
        let (translation, rotation) = local_trs(rest_gltf, bone.parent, index);
        out.nodes.push(json!({
            "name": bone.name,
            "translation": translation,
            "rotation": rotation,
            "extras": {"iw4l": {"bone": index, "model": bone.model}},
        }));
        if bone.parent.is_none() {
            roots.push(base + index);
        }
    }
    for (index, bone) in dobj.bones.iter().enumerate() {
        if let Some(parent) = bone.parent {
            push_child(&mut out.nodes[base + parent], base + index);
        }
    }
    BoneNodes { roots }
}

fn push_child(node: &mut Value, child: usize) {
    match node.get_mut("children").and_then(Value::as_array_mut) {
        Some(children) => children.push(json!(child)),
        None => node["children"] = json!([child]),
    }
}

fn surface_material(
    out: &mut GltfBuilder,
    prepared: &PreparedMatch,
    entry: &asset_model::FpvMeshEntry,
    surface: usize,
) -> Option<usize> {
    let Some(canonical) = entry
        .material_edges
        .get(surface)
        .and_then(|edge| edge.bound_index())
    else {
        out.refuse("MaterialUnbound");
        return Some(untextured_material(out));
    };
    match out.material(canonical, &prepared.materials.population) {
        Ok(material) => Some(material),
        Err("MaterialBlendUnsupported") => {
            out.refuse("MaterialBlendUnsupported");
            None
        }
        Err(reason) => {
            out.refuse(reason);
            Some(untextured_material(out))
        }
    }
}

fn untextured_material(out: &mut GltfBuilder) -> usize {
    if let Some(index) = out
        .materials
        .iter()
        .position(|material| material["name"] == "iw4l_untextured")
    {
        return index;
    }
    out.materials.push(json!({
        "name": "iw4l_untextured",
        "pbrMetallicRoughness": {
            "baseColorFactor": [0.5, 0.5, 0.5, 1.0],
            "metallicFactor": 0.0,
            "roughnessFactor": 1.0,
        },
    }));
    out.materials.len() - 1
}

fn skinned_primitive(
    out: &mut GltfBuilder,
    entry: &asset_model::FpvMeshEntry,
    surface: usize,
    bone_base: usize,
    bake: &[Mat4],
    material: usize,
) -> Result<Value, &'static str> {
    let skel = &entry.skel;
    let (vertex_start, vertex_count) = *skel
        .surface_vertex_ranges
        .get(surface)
        .ok_or("GeometryAttributeMissing")?;
    let (index_start, index_count) = *skel
        .surface_index_ranges
        .get(surface)
        .ok_or("GeometryIndexMissing")?;
    let vertex_end = vertex_start + vertex_count;
    if vertex_count == 0 || vertex_end > skel.positions.len() {
        return Err("GeometryAttributeMissing");
    }
    let indices = skel
        .indices
        .get(index_start..index_start + index_count)
        .ok_or("GeometryIndexMissing")?;
    if indices.is_empty() || indices.len() % 3 != 0 {
        return Err("GeometryIndexInvalid");
    }
    let mut local_indices = Vec::with_capacity(indices.len());
    for &index in indices {
        let index = index as usize;
        if !(vertex_start..vertex_end).contains(&index) {
            return Err("GeometryIndexOutOfRange");
        }
        local_indices.push((index - vertex_start) as u32);
    }

    let conversion = basis_conversion();
    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut joints = Vec::with_capacity(vertex_count);
    let mut weights = Vec::with_capacity(vertex_count);
    for vertex in vertex_start..vertex_end {
        let (bones, influence) = vertex_influences(skel.vert_skin.get(vertex), bone_base, bake.len());
        let position = Vec3::from_array(skel.positions[vertex]);
        let normal = skel
            .normals
            .get(vertex)
            .map_or(Vec3::Z, |normal| Vec3::from_array(*normal));
        let mut baked_position = Vec3::ZERO;
        let mut baked_normal = Vec3::ZERO;
        for (&bone, &weight) in bones.iter().zip(&influence) {
            if weight <= 0.0 {
                continue;
            }
            let matrix = bake[usize::from(bone)];
            baked_position += weight * matrix.transform_point3(position);
            baked_normal += weight * matrix.transform_vector3(normal);
        }
        let gltf_normal = (conversion * baked_normal).normalize_or(Vec3::Y);
        let gltf_position = conversion * baked_position * INCHES_TO_METERS;
        if !gltf_position.is_finite() {
            return Err("GeometryNonFinite");
        }
        positions.push(gltf_position.to_array());
        normals.push(gltf_normal.to_array());
        joints.push(bones);
        weights.push(influence);
    }

    let mut attributes = serde_json::Map::new();
    attributes.insert(
        "POSITION".into(),
        json!(out.vec3_accessor(&positions, true)),
    );
    attributes.insert("NORMAL".into(), json!(out.vec3_accessor(&normals, false)));
    if let Some(uvs) = skel.uvs.get(vertex_start..vertex_end)
        && uvs.iter().flatten().all(|value| value.is_finite())
    {
        attributes.insert("TEXCOORD_0".into(), json!(out.vec2_accessor(uvs)));
    }
    attributes.insert("JOINTS_0".into(), json!(joints_accessor(out, &joints)));
    attributes.insert("WEIGHTS_0".into(), json!(out.vec4_accessor(&weights)));
    let index_accessor = out.index_accessor(&local_indices);
    out.primitives += 1;
    out.triangles += (local_indices.len() / 3) as u64;
    Ok(json!({
        "attributes": attributes,
        "indices": index_accessor,
        "material": material,
        "mode": 4,
    }))
}

fn vertex_influences(
    skin: Option<&asset_model::VertSkin>,
    bone_base: usize,
    bone_count: usize,
) -> ([u16; 4], [f32; 4]) {
    let rigid = ([bone_base as u16, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]);
    let Some(skin) = skin else {
        return rigid;
    };
    let mut bones = [0u16; 4];
    let mut weights = [0.0f32; 4];
    for slot in 0..4 {
        let weight = skin.weights[slot];
        let bone = bone_base + usize::from(skin.bones[slot]);
        if weight > 0.0 && weight.is_finite() && bone < bone_count {
            bones[slot] = bone as u16;
            weights[slot] = weight;
        }
    }
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return rigid;
    }
    (bones, weights.map(|weight| weight / total))
}

fn joints_accessor(out: &mut GltfBuilder, values: &[[u16; 4]]) -> usize {
    out.align_bin();
    let offset = out.bin.len();
    for value in values.iter().flatten() {
        out.bin.extend_from_slice(&value.to_le_bytes());
    }
    let view = out.push_view(offset, out.bin.len() - offset, TARGET_ARRAY_BUFFER);
    out.accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_U16,
        "count": values.len(),
        "type": "VEC4",
    }));
    out.accessors.len() - 1
}

fn data_accessor(out: &mut GltfBuilder, values: &[f32], count: usize, kind: &str) -> usize {
    out.align_bin();
    let offset = out.bin.len();
    for value in values {
        out.bin.extend_from_slice(&value.to_le_bytes());
    }
    let view = out.buffer_views.len();
    out.buffer_views.push(json!({
        "buffer": 0,
        "byteOffset": offset,
        "byteLength": out.bin.len() - offset,
    }));
    out.accessors.push(json!({
        "bufferView": view,
        "componentType": COMPONENT_F32,
        "count": count,
        "type": kind,
    }));
    out.accessors.len() - 1
}

fn animation_json(out: &mut GltfBuilder, dobj: &DObj, clip: &AnimClip, name: &str) -> Value {
    let frames = if clip.framerate > 0.0 {
        usize::from(clip.numframes)
    } else {
        0
    };
    let times = (0..=frames)
        .map(|frame| {
            if clip.framerate > 0.0 {
                frame as f32 / clip.framerate
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    let poses = times
        .iter()
        .map(|&time| {
            sample_pose(dobj, clip, time)
                .into_iter()
                .map(to_gltf)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let input = data_accessor(out, &times, times.len(), "SCALAR");
    let last = times.last().copied().unwrap_or(0.0);
    out.accessors[input]["min"] = json!([0.0]);
    out.accessors[input]["max"] = json!([last]);

    let mut samplers = Vec::new();
    let mut channels = Vec::new();
    for (bone, info) in dobj.bones.iter().enumerate() {
        let mut translations = Vec::with_capacity(poses.len() * 3);
        let mut rotations = Vec::with_capacity(poses.len() * 4);
        let mut previous: Option<Quat> = None;
        for pose in &poses {
            let (translation, rotation) = local_trs(pose, info.parent, bone);
            let mut rotation = Quat::from_array(rotation);
            if let Some(previous) = previous
                && previous.dot(rotation) < 0.0
            {
                rotation = -rotation;
            }
            previous = Some(rotation);
            translations.extend_from_slice(&translation);
            rotations.extend_from_slice(&rotation.to_array());
        }
        for (path, values, kind) in [
            ("translation", &translations, "VEC3"),
            ("rotation", &rotations, "VEC4"),
        ] {
            let output = data_accessor(out, values, poses.len(), kind);
            channels.push(json!({
                "sampler": samplers.len(),
                "target": {"node": bone, "path": path},
            }));
            samplers.push(json!({
                "input": input,
                "output": output,
                "interpolation": "LINEAR",
            }));
        }
    }
    json!({
        "name": name,
        "samplers": samplers,
        "channels": channels,
        "extras": {"iw4l": {
            "clip": clip.name,
            "framerate": clip.framerate,
            "numframes": clip.numframes,
            "looping": clip.looping,
        }},
    })
}

fn sample_pose(dobj: &DObj, clip: &AnimClip, time: f32) -> Vec<Mat4> {
    let tracks = dobj.tracks_for(clip);
    let instance = AnimInstance {
        clip,
        tracks: &tracks,
        time,
        weight: 1.0,
        parts: None,
    };
    dobj.pose(&[instance], &dobj.all_parts(), Mat4::IDENTITY)
}

fn to_gltf(world: Mat4) -> Mat4 {
    let scale = Mat4::from_mat3(basis_conversion() * INCHES_TO_METERS);
    scale * world * scale.inverse()
}

fn local_trs(worlds: &[Mat4], parent: Option<usize>, bone: usize) -> ([f32; 3], [f32; 4]) {
    let local = match parent {
        Some(parent) => worlds[parent].inverse() * worlds[bone],
        None => worlds[bone],
    };
    let (_, rotation, translation) = local.to_scale_rotation_translation();
    (translation.to_array(), rotation.normalize().to_array())
}

fn slot_name(slot: WeaponAnimSlot) -> String {
    let debug = format!("{slot:?}");
    let mut name = String::with_capacity(debug.len() + 4);
    for (index, character) in debug.chars().enumerate() {
        if character.is_ascii_uppercase() && index > 0 {
            name.push('_');
        }
        name.push(character.to_ascii_lowercase());
    }
    name
}
