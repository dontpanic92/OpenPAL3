use std::{cell::RefCell, collections::HashMap, path::Path, rc::Rc};

use anyhow::{Context, Result, bail, ensure};
use common::store_ext::StoreExt2;
use crosscom::ComRc;
use encoding::Encoding;
use fileformats::pal3::msh::{self, Mesh, Motion};
use ini::Ini;
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    comdef::{IAnimatedMeshComponent, IEntity, IEntityExt},
    components::mesh::{AnimatedMeshComponent, Geometry, MorphTarget, TexCoord},
    math::{Mat44, Quaternion, Vec3},
    rendering::{ComponentFactory, Pal3ActorMaterialDef, VertexBuffer, VertexComponents},
};

struct Skin {
    mesh: Mesh,
    motion: Motion,
    tracks: Vec<Option<usize>>,
    parents: Vec<Option<usize>>,
    inverse_bind: Vec<Mat44>,
    scratch: RefCell<Scratch>,
}

struct Scratch {
    time: Option<u32>,
    world: Vec<Mat44>,
    skin: Vec<Mat44>,
    positions: Vec<Vec<Vec3>>,
    normals: Vec<Vec<Vec3>>,
}

fn point(matrix: &Mat44, value: &[f32; 3], translation: bool) -> Vec3 {
    let mut result = [0.; 3];
    for row in 0..3 {
        result[row] = (0..3).map(|col| matrix[row][col] * value[col]).sum::<f32>()
            + if translation { matrix[row][3] } else { 0. };
    }
    Vec3::new(result[0], result[1], result[2])
}

fn sample(track: &msh::Track, time: u32) -> ([f32; 3], Quaternion) {
    let next = track.frames.partition_point(|key| key.time_ms <= time);
    let key = &track.frames[next.saturating_sub(1)];
    let quaternion = |q: [f32; 4]| Quaternion::new(q[0], q[1], q[2], q[3]);
    if next == 0 || next == track.frames.len() {
        let last = track.frames.last().unwrap();
        return (last.position, quaternion(last.rotation));
    }
    let to = &track.frames[next];
    let previous_weight = (to.time_ms - time) as f32 / (to.time_ms - key.time_ms) as f32;
    let previous_weight = f64::from(previous_weight);
    let mut previous = key.rotation.map(f64::from);
    let next = to.rotation.map(f64::from);
    let mut dot = (0..4).rev().map(|i| previous[i] * next[i]).sum::<f64>() as f32;
    if dot < 0. {
        dot = -dot;
        for value in &mut previous {
            *value = -*value;
        }
    }
    // Native gbQuaternion interpolation keeps the exported quaternion lengths,
    // uses a 1e-5 cutoff, and does not normalize its result.
    let (next_weight, rotation_previous_weight) = if 1. - f64::from(dot) > 1e-5 {
        let degrees = (f64::from(dot).clamp(-1., 1.).acos() * f64::from(57.295776_f32)) as f32;
        let radians = f64::from(degrees) * f64::from(0.017453292_f32);
        let denominator = radians.sin() as f32;
        (
            ((1. - previous_weight) * radians).sin() / f64::from(denominator),
            (previous_weight * radians).sin() / f64::from(denominator),
        )
    } else {
        (1. - previous_weight, previous_weight)
    };
    let rotation = std::array::from_fn(|i| {
        (next[i] * next_weight + previous[i] * rotation_previous_weight) as f32
    });
    (
        std::array::from_fn(|i| {
            (f64::from(to.position[i]) * (1. - previous_weight)
                + f64::from(key.position[i]) * previous_weight) as f32
        }),
        quaternion(rotation),
    )
}

fn rotation_matrix(q: Quaternion) -> Mat44 {
    let [x, y, z, w] = [q.x, q.y, q.z, q.w].map(f64::from);
    let [xx, yy, zz, xy, xz, yz] = [
        2. * x * x,
        2. * y * y,
        2. * z * z,
        2. * x * y,
        2. * x * z,
        2. * y * z,
    ]
    .map(|value| f64::from(value as f32));
    let (wx, wy, wz) = (2. * w * x, 2. * w * y, 2. * w * z);
    let mut result = Mat44::new_identity();
    for (row, values) in [
        [1. - yy - zz, xy - wz, xz + wy],
        [xy + wz, 1. - xx - zz, yz - wx],
        [xz - wy, yz + wx, 1. - xx - yy],
    ]
    .into_iter()
    .enumerate()
    {
        for (col, value) in values.into_iter().enumerate() {
            result[row][col] = value as f32;
        }
    }
    result
}

impl Skin {
    fn new(mesh: Mesh, motion: Motion) -> Result<Self> {
        let bones: HashMap<_, _> = mesh
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.bone >= 0)
            .map(|(i, node)| (node.bone, i))
            .collect();
        let names: HashMap<_, _> = motion
            .tracks
            .iter()
            .enumerate()
            .map(|(i, track)| (track.name.as_str(), i))
            .collect();
        let tracks = mesh
            .nodes
            .iter()
            .map(|n| names.get(n.name.as_str()).copied())
            .collect();
        let mut parents = vec![];
        let mut inverse_bind = vec![];
        for (index, node) in mesh.nodes.iter().enumerate() {
            let parent = bones.get(&node.parent_bone).copied();
            ensure!(
                parent.is_none_or(|parent| parent < index),
                "Unordered PAL3 bone hierarchy"
            );
            parents.push(parent);
            let mut matrix = Mat44::new_zero();
            for row in 0..4 {
                for col in 0..4 {
                    matrix[row][col] = node.inverse_bind[col * 4 + row];
                }
            }
            inverse_bind.push(matrix);
        }
        // Resolve file bone IDs once; the hot path uses node indices.
        let mut mesh = mesh;
        for section in &mut mesh.sections {
            for vertex in &mut section.vertices {
                for (bone, _) in &mut vertex.influences {
                    let index = bones[&i32::from(*bone)];
                    ensure!(index <= u8::MAX as usize, "Too many PAL3 skin nodes");
                    *bone = index as u8;
                }
            }
        }
        let scratch = Scratch {
            time: None,
            world: vec![Mat44::new_identity(); mesh.nodes.len()],
            skin: vec![Mat44::new_identity(); mesh.nodes.len()],
            positions: mesh
                .sections
                .iter()
                .map(|s| vec![Vec3::new_zeros(); s.vertices.len()])
                .collect(),
            normals: mesh
                .sections
                .iter()
                .map(|s| vec![Vec3::new_zeros(); s.faces.len() * 3])
                .collect(),
        };
        Ok(Self {
            mesh,
            motion,
            tracks,
            parents,
            inverse_bind,
            scratch: RefCell::new(scratch),
        })
    }

    fn update(&self, time: u32) {
        let mut scratch = self.scratch.borrow_mut();
        if scratch.time == Some(time) {
            return;
        }
        for (index, node) in self.mesh.nodes.iter().enumerate() {
            if node.bone < 0 {
                continue;
            }
            let (position, mut rotation) = self.tracks[index]
                .map(|track| sample(&self.motion.tracks[track], time))
                .unwrap_or_else(|| {
                    (
                        node.position,
                        Quaternion::new(
                            node.rotation[0],
                            node.rotation[1],
                            node.rotation[2],
                            node.rotation[3],
                        ),
                    )
                });
            // Native node evaluation negates W before composing the local
            // rotation; disk matrices use translation in the final row.
            rotation.w = -rotation.w;
            let mut local = rotation_matrix(rotation);
            for row in 0..3 {
                local[row][3] = position[row];
            }
            scratch.world[index] = self.parents[index]
                .map(|parent| Mat44::multiplied(&scratch.world[parent], &local))
                .unwrap_or(local);
            scratch.skin[index] =
                Mat44::multiplied(&scratch.world[index], &self.inverse_bind[index]);
        }
        for (section_index, section) in self.mesh.sections.iter().enumerate() {
            for (index, vertex) in section.vertices.iter().enumerate() {
                let mut position = Vec3::new_zeros();
                if vertex.influences.is_empty() {
                    position =
                        Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                } else {
                    for &(bone, weight) in &vertex.influences {
                        let matrix = &scratch.skin[bone as usize];
                        let weight = f32::from(weight) / 255.;
                        position = Vec3::add(
                            &position,
                            &Vec3::scalar_mul(weight, &point(matrix, &vertex.position, true)),
                        );
                    }
                }
                scratch.positions[section_index][index] = position;
            }
            for (face_index, face) in section.faces.iter().enumerate() {
                for corner in 0..3 {
                    let vertex = &section.vertices[face.indices[corner] as usize];
                    let normal = face.normals[corner];
                    let mut value = Vec3::new_zeros();
                    if vertex.influences.is_empty() {
                        value = Vec3::new(normal[0], normal[1], normal[2]);
                    } else {
                        for &(bone, weight) in &vertex.influences {
                            value = Vec3::add(
                                &value,
                                &Vec3::scalar_mul(
                                    f32::from(weight) / 255.,
                                    &point(&scratch.skin[bone as usize], &normal, false),
                                ),
                            );
                        }
                    }
                    scratch.normals[section_index][face_index * 3 + corner] = value;
                }
            }
        }
        scratch.time = Some(time);
    }

    fn write(&self, seconds: f32, looping: bool, section: usize, buffer: &mut VertexBuffer) {
        let time = (seconds.max(0.) as f64 * 1000.).trunc() as u32;
        let duration = self.motion.duration_ms;
        let at_end = seconds >= duration as f32 / 1000.;
        let time = if looping && duration != 0 {
            if at_end && time < duration {
                0
            } else {
                time % duration
            }
        } else if at_end {
            duration.saturating_sub(4)
        } else {
            time
        };
        self.update(time);
        let scratch = self.scratch.borrow();
        for (face_index, face) in self.mesh.sections[section].faces.iter().enumerate() {
            for corner in 0..3 {
                let vertex = face.indices[corner] as usize;
                buffer.set_component(
                    face_index * 3 + corner,
                    VertexComponents::POSITION,
                    |value: &mut Vec3| *value = scratch.positions[section][vertex],
                );
                buffer.set_component(
                    face_index * 3 + corner,
                    VertexComponents::NORMAL,
                    |value: &mut Vec3| *value = scratch.normals[section][face_index * 3 + corner],
                );
            }
        }
    }
}

pub(crate) fn create_animation_from_msh(
    entity: ComRc<IEntity>,
    factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    folder: &Path,
    action: &str,
) -> Result<ComRc<IAnimatedMeshComponent>> {
    let mut configurations = vec![];
    for entry in vfs.entries(folder)? {
        let entry = entry?;
        let name = Path::new(&entry.name);
        if name
            .extension()
            .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("ini"))
        {
            configurations.push(folder.join(name.file_name().context("Actor configuration name")?));
        }
    }
    configurations.sort();
    let mut selected = None;
    for path in configurations {
        let data = vfs.read_to_end(&path)?;
        let text = encoding::all::GBK
            .decode(&data, encoding::DecoderTrap::Ignore)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let config = Ini::load_from_str(&text)?;
        let Some(actor) = config.section(Some("actor")) else {
            continue;
        };
        for (section, values) in &config {
            if section.is_some_and(|name| name.starts_with("action_"))
                && values
                    .get("name")
                    .is_some_and(|name| name.trim().eq_ignore_ascii_case(action))
            {
                selected = Some((
                    actor.get("mesh").context("Actor mesh")?.trim().to_owned(),
                    actor
                        .get("material")
                        .context("Actor material")?
                        .trim()
                        .to_owned(),
                    values
                        .get("file")
                        .context("Actor motion")?
                        .trim()
                        .to_owned(),
                ));
                break;
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let Some((mesh, material, motion)) = selected else {
        bail!("No MSH action {action}")
    };
    let skin = Rc::new(Skin::new(
        msh::read_mesh(&vfs.read_to_end(folder.join(mesh))?)?,
        msh::read_motion(&vfs.read_to_end(folder.join(motion))?)?,
    )?);
    let materials = msh::read_materials(&vfs.read_to_end(folder.join(material))?)?;
    skin.update(0);
    let mut geometries = vec![];
    {
        let scratch = skin.scratch.borrow();
        for (section_index, section) in skin.mesh.sections.iter().enumerate() {
            ensure!(
                !section.vertices.is_empty() && !section.faces.is_empty(),
                "Empty MSH section"
            );
            let material = materials
                .get(section.material)
                .context("MSH material index")?;
            let texture = folder.join(
                material
                    .textures
                    .first()
                    .filter(|name| !name.is_empty())
                    .context("MSH texture")?,
            );
            let definition = Pal3ActorMaterialDef::create(
                texture.to_str().context("MSH texture path")?,
                |path| vfs.open(path).ok(),
            );
            let mut params = *definition.params();
            for i in 0..4 {
                params.tint[i] *= material.diffuse[i];
            }
            for i in 0..3 {
                params.ambient[i] *= material.ambient[i];
            }
            let mut positions = vec![];
            let mut normals = vec![];
            let mut uv = vec![];
            for (face_index, face) in section.faces.iter().enumerate() {
                for corner in 0..3 {
                    let vertex = face.indices[corner] as usize;
                    positions.push(scratch.positions[section_index][vertex]);
                    normals.push(scratch.normals[section_index][face_index * 3 + corner]);
                    // Native actor images stay bottom-up; TextureStore normalizes
                    // their rows, so use the same V conversion as MV3 actors.
                    uv.push(TexCoord::new(face.uv[corner][0], -face.uv[corner][1]));
                }
            }
            let indices = (0..positions.len() as u32).collect();
            geometries.push(Geometry::new(
                &positions,
                Some(&normals),
                &[uv],
                indices,
                definition.with_params(params),
            ));
        }
    }
    ensure!(!geometries.is_empty(), "Empty MSH actor");
    log::debug!(
        "PAL3 MSH actor {}: {}, action={}, bones={}, vertices={}, faces={}",
        entity.name(),
        folder.display(),
        action,
        skin.mesh.nodes.iter().filter(|node| node.bone >= 0).count(),
        skin.mesh
            .sections
            .iter()
            .map(|s| s.vertices.len())
            .sum::<usize>(),
        skin.mesh
            .sections
            .iter()
            .map(|s| s.faces.len())
            .sum::<usize>(),
    );
    let animation = AnimatedMeshComponent::new(entity, factory.clone());
    let duration = skin.motion.duration_ms as f32 / 1000.;
    let hold = skin
        .motion
        .events
        .iter()
        .find(|(_, event)| event.starts_with("hold"))
        .map_or(0., |(time, _)| *time as f32 / 1000.);
    animation.set_morph_targets(
        vec![
            MorphTarget::new(geometries.clone(), 0., factory.clone()),
            MorphTarget::new(geometries, duration, factory.clone()),
        ],
        hold,
    );
    let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
    for section in &skin.mesh.sections {
        for vertex in &section.vertices {
            min.x = min.x.min(vertex.position[0]);
            min.y = min.y.min(vertex.position[1]);
            min.z = min.z.min(vertex.position[2]);
            max.x = max.x.max(vertex.position[0]);
            max.y = max.y.max(vertex.position[1]);
            max.z = max.z.max(vertex.position[2]);
        }
    }
    animation.set_model_bounds((min, max));
    animation.set_vertex_sampler(Rc::new(move |time, looping, section, buffer| {
        skin.write(time, looping, section, buffer)
    }));
    Ok(ComRc::from_object(animation))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [f32; 16] {
        [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ]
    }

    fn triangle(bone: u8) -> msh::Section {
        msh::Section {
            material: 0,
            vertices: [[1., 0., 0.], [0., 1., 0.], [0., 0., 0.]]
                .map(|position| msh::Vertex {
                    position,
                    influences: vec![(bone, 255)],
                })
                .to_vec(),
            faces: vec![msh::Face {
                indices: [0, 1, 2],
                normals: [[0., 0., 1.]; 3],
                uv: [[0.; 2]; 3],
            }],
        }
    }

    fn node(name: &str, bone: i32, parent_bone: i32) -> msh::Node {
        msh::Node {
            name: name.into(),
            position: [0.; 3],
            rotation: [0., 0., 0., 1.],
            inverse_bind: identity(),
            bone,
            parent_bone,
        }
    }

    #[test]
    fn skinning_interpolates_bone_rotation_not_baked_vertex_positions() {
        let skin = Skin::new(
            Mesh {
                nodes: vec![node("root", 0, -1)],
                sections: vec![triangle(0)],
            },
            Motion {
                tracks: vec![msh::Track {
                    name: "root".into(),
                    frames: vec![
                        msh::KeyFrame {
                            time_ms: 0,
                            position: [0.; 3],
                            rotation: [0., 0., 0., 1.],
                        },
                        msh::KeyFrame {
                            time_ms: 1000,
                            position: [2., 0., 0.],
                            rotation: [
                                0.,
                                0.,
                                std::f32::consts::FRAC_1_SQRT_2,
                                std::f32::consts::FRAC_1_SQRT_2,
                            ],
                        },
                    ],
                }],
                events: vec![],
                duration_ms: 1000,
            },
        )
        .unwrap();
        skin.update(500);
        let p = skin.scratch.borrow().positions[0][0];
        assert!((p.x - (1. + std::f32::consts::FRAC_1_SQRT_2)).abs() < 1e-5);
        assert!((p.y + std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        let n = skin.scratch.borrow().normals[0][0];
        assert!((n.norm() - 1.).abs() < 1e-5);
    }

    #[test]
    fn skinning_resolves_file_bone_ids_parent_transforms_and_inverse_bind() {
        let mut parent = node("parent", 10, -1);
        parent.position = [3., 0., 0.];
        let mut child = node("child", 4, 10);
        child.position = [0., 2., 0.];
        child.inverse_bind[12] = -3.;
        child.inverse_bind[13] = -2.;
        let skin = Skin::new(
            Mesh {
                nodes: vec![parent, child],
                sections: vec![triangle(4)],
            },
            Motion {
                tracks: vec![msh::Track {
                    name: "parent".into(),
                    frames: vec![
                        msh::KeyFrame {
                            time_ms: 0,
                            position: [3., 0., 0.],
                            rotation: [0., 0., 0., 1.],
                        },
                        msh::KeyFrame {
                            time_ms: 1000,
                            position: [4., 0., 0.],
                            rotation: [0., 0., 0., 1.],
                        },
                    ],
                }],
                events: vec![],
                duration_ms: 1000,
            },
        )
        .unwrap();
        skin.update(500);
        let p = skin.scratch.borrow().positions[0][0];
        assert!((p.x - 1.5).abs() < 1e-5 && p.y.abs() < 1e-5);
    }

    #[test]
    #[ignore = "requires local original PAL3 assets and PAL3_MSH_ORACLE"]
    fn original_msh_pose_oracle() {
        let path = std::env::var("PAL3_ASSET_PATH").unwrap();
        let vfs = packfs::init_virtual_fs(Path::new(&path), None);
        let oracle: serde_json::Value = serde_json::from_slice(
            &std::fs::read(std::env::var("PAL3_MSH_ORACLE").unwrap()).unwrap(),
        )
        .unwrap();
        for case in oracle["cases"].as_array().unwrap() {
            let role = case["role"].as_str().unwrap();
            let folder = format!("/basedata/basedata/ROLE/{role}");
            let mesh = msh::read_mesh(
                &vfs.read_to_end(format!("{folder}/{}", case["mesh"].as_str().unwrap()))
                    .unwrap(),
            )
            .unwrap();
            let motion = msh::read_motion(
                &vfs.read_to_end(format!("{folder}/{}.MOV", case["action"].as_str().unwrap()))
                    .unwrap(),
            )
            .unwrap();
            for (section, data) in mesh.sections.iter().enumerate() {
                for (index, face) in data.faces.iter().enumerate() {
                    for corner in 0..3 {
                        let expected = &case["attributes"][section][index * 3 + corner];
                        for axis in 0..3 {
                            assert!(
                                (face.normals[corner][axis]
                                    - expected[axis].as_f64().unwrap() as f32)
                                    .abs()
                                    < 1e-6,
                                "{role} face={index} normal axis={axis}"
                            );
                        }
                        for axis in 0..2 {
                            assert!(
                                (face.uv[corner][axis]
                                    - expected[axis + 3].as_f64().unwrap() as f32)
                                    .abs()
                                    < 1e-7,
                                "{role} face={index} UV axis={axis}"
                            );
                        }
                    }
                }
            }
            let skin = Skin::new(mesh, motion).unwrap();
            for frame in case["samples"].as_array().unwrap() {
                let time = frame["time_ms"].as_u64().unwrap() as u32;
                skin.update(time);
                let scratch = skin.scratch.borrow();
                for (section, expected) in frame["vertices"].as_array().unwrap().iter().enumerate()
                {
                    for (index, position) in expected.as_array().unwrap().iter().enumerate() {
                        let actual = scratch.positions[section][index];
                        for (axis, value) in [actual.x, actual.y, actual.z].into_iter().enumerate()
                        {
                            assert!(
                                (value - position[axis].as_f64().unwrap() as f32).abs() < 0.003,
                                "{role} {} t={time} vertex={index} axis={axis}: {value} vs {}",
                                case["action"],
                                position[axis]
                            );
                        }
                    }
                }
                for (index, node) in skin
                    .mesh
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.bone >= 0)
                {
                    let expected = &frame["matrices"][node.bone.to_string()];
                    for row in 0..4 {
                        for col in 0..4 {
                            assert!(
                                (scratch.skin[index][row][col]
                                    - expected[col * 4 + row].as_f64().unwrap() as f32)
                                    .abs()
                                    < 0.0001,
                                "{role} {} t={time} bone={} matrix[{row}][{col}]: {} vs {}",
                                case["action"],
                                node.bone,
                                scratch.skin[index][row][col],
                                expected[col * 4 + row]
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires original game assets via PAL3_ASSET_PATH"]
    fn original_msh_actors_load_all_configured_actions_and_deform() {
        use crate::openpal3::{asset_manager::AssetManager, scene::mechanism_tests::UnusedFactory};
        use radiance::comdef::{IAnimatedMeshComponentExt, IComponentImpl, IEntityExt};
        let path = std::env::var("PAL3_ASSET_PATH").unwrap();
        let vfs = Rc::new(packfs::init_virtual_fs(Path::new(&path), None));
        let factory: Rc<dyn ComponentFactory> = Rc::new(UnusedFactory);
        let assets = Rc::new(AssetManager::new(factory.clone(), vfs.clone()));
        let entity = assets
            .load_role("252", "", "ROLE_16".into(), false)
            .unwrap();
        let controller = super::super::RoleController::get_role_controller(entity.clone()).unwrap();
        assert!(
            controller
                .inner::<super::super::RoleController>()
                .try_play_anim("z3", super::super::RoleAnimationRepeatMode::Repeat(1))
        );
        let mut actions = std::collections::BTreeSet::new();
        for role in vfs.entries("/basedata/basedata/ROLE").unwrap() {
            let role = role.unwrap().name;
            let folder = Path::new("/basedata/basedata/ROLE").join(&role);
            let Ok(entries) = vfs.entries(&folder) else {
                continue;
            };
            let entries: Vec<_> = entries.map(|entry| entry.unwrap()).collect();
            if !entries.iter().any(|e| {
                Path::new(&e.name)
                    .extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("msh"))
            }) {
                continue;
            }
            for entry in &entries {
                if !Path::new(&entry.name)
                    .extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("ini"))
                {
                    continue;
                }
                let text = vfs.read_to_end_from_gbk(folder.join(&entry.name)).unwrap();
                let config = Ini::load_from_str(&text).unwrap();
                if config.section(Some("actor")).is_none() {
                    continue;
                }
                for (section, values) in &config {
                    if section.is_some_and(|name| name.starts_with("action_")) {
                        actions.insert((
                            role.to_string_lossy().to_string(),
                            values.get("name").unwrap().trim().to_string(),
                        ));
                    }
                }
            }
        }
        eprintln!("Testing {} original MSH/MOV actions", actions.len());
        for (role, action) in &actions {
            let entity = radiance::scene::CoreEntity::create("test".into(), true);
            let animation = create_animation_from_msh(
                entity.clone(),
                &factory,
                &vfs,
                Path::new(&format!("/basedata/basedata/ROLE/{role}")),
                action,
            )
            .unwrap();
            let component = animation.inner::<AnimatedMeshComponent>();
            let vertices = || {
                let result = RefCell::new(vec![]);
                entity.get_rendering_component().unwrap().render_objects()[0]
                    .as_dyn()
                    .update_vertices(&|buffer| {
                        *result.borrow_mut() = (0..buffer.count())
                            .map(|i| {
                                let position = buffer.position(i).unwrap();
                                let normal = buffer.normal(i).unwrap();
                                let uv = buffer.tex_coord(i).unwrap();
                                [
                                    position.x, position.y, position.z, normal.x, normal.y,
                                    normal.z, uv.x, uv.y,
                                ]
                            })
                            .collect();
                    });
                result.into_inner()
            };
            let before = vertices();
            component.on_updating(0.5);
            component.on_updating(0.1);
            let after = vertices();
            assert!(after.iter().flatten().all(|v| v.is_finite()));
            assert!(before.iter().zip(&after).all(|(a, b)| a[6..] == b[6..]));
            if role == "252" && action == "z1" {
                assert_ne!(before, after, "NPC252 idle must deform its actual vertices");
            }
            component.set_loop_playback(true);
            animation.play(true);
            component.on_updating(component.morph_animation_length().unwrap());
            component.on_updating(0.);
            assert_eq!(
                before,
                vertices(),
                "{role} {action} loop seam must return to its first pose"
            );
            assert!(
                animation.morph_animation_state()
                    == radiance::components::mesh::MorphAnimationState::Playing
            );
        }
    }
}
