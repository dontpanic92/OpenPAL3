use crate::loaders::mv3_loader::*;
use radiance::math::{Vec2, Vec3};
use radiance::rendering::{RenderObject, SimpleMaterial, VertexBuffer, VertexComponents};
use radiance::scene::{CoreEntity, EntityExtension};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(PartialEq)]
pub enum Mv3AnimRepeatMode {
    NoRepeat,
    Repeat,
}

pub struct Mv3ModelEntity {
    texture_path: PathBuf,
    vertices: Vec<VertexBuffer>,
    indices: Vec<u32>,
    anim_timestamps: Vec<u32>,
    last_anim_time: u32,
    repeat_mode: Mv3AnimRepeatMode,
    anim_finished: bool,
}

impl Mv3ModelEntity {
    pub fn new_from_file(path: &str, anim_repeat_mode: Mv3AnimRepeatMode) -> Self {
        let mv3file = mv3_load_from_file(&path).unwrap();
        let model: &Mv3Model = &mv3file.models[0];
        let mesh: &Mv3Mesh = &model.meshes[0];

        let mut texture_path = PathBuf::from(path);
        texture_path.pop();
        texture_path.push(std::str::from_utf8(&mv3file.textures[0].names[0]).unwrap());

        let hash =
            |index, texcoord_index| index as u32 * model.texcoord_count + texcoord_index as u32;

        let mut indices: Vec<u32> = Vec::<u32>::with_capacity(model.vertex_per_frame as usize);
        let mut vertices_data: Vec<Vec<(Vec3, Vec2)>> = vec![vec![]; model.frame_count as usize];
        let mut index_map = HashMap::new();

        for t in &mesh.triangles {
            for (&i, &j) in t.indices.iter().zip(&t.texcoord_indices) {
                let h = hash(i, j);
                let index = match index_map.get(&h) {
                    None => {
                        let index = index_map.len();
                        for k in 0..model.frame_count as usize {
                            let frame = &model.frames[k];
                            vertices_data[k].push((
                                Vec3::new(
                                    frame.vertices[i as usize].x as f32 * 0.01562,
                                    frame.vertices[i as usize].y as f32 * 0.01562,
                                    frame.vertices[i as usize].z as f32 * 0.01562,
                                ),
                                Vec2::new(
                                    model.texcoords[j as usize].u,
                                    -model.texcoords[j as usize].v,
                                ),
                            ));
                        }
                        index_map.insert(h, index as u32);
                        index as u32
                    }
                    Some(index) => *index,
                };

                indices.push(index);
            }
        }

        let mut vertices: Vec<VertexBuffer> =
            Vec::<VertexBuffer>::with_capacity(model.frame_count as usize);
        for i in 0..model.frame_count as usize {
            vertices.push(VertexBuffer::new(
                VertexComponents::POSITION | VertexComponents::TEXCOORD,
                index_map.len(),
            ));

            let vertex_data = &vertices_data[i];
            let vert = vertices.get_mut(i).unwrap();
            for j in 0..vertex_data.len() {
                vert.set_component(j, VertexComponents::POSITION, |p: &mut Vec3| {
                    *p = vertex_data[j].0;
                });
                vert.set_component(j, VertexComponents::TEXCOORD, |t: &mut Vec2| {
                    *t = vertex_data[j].1;
                });
            }
        }

        let anim_timestamps = model.frames.iter().map(|f| f.timestamp).collect();

        Mv3ModelEntity {
            texture_path,
            anim_timestamps,
            last_anim_time: 0,
            vertices,
            indices,
            repeat_mode: anim_repeat_mode,
            anim_finished: false,
        }
    }

    pub fn anim_finished(&self) -> bool {
        self.anim_finished
    }
}

impl EntityExtension<Mv3ModelEntity> for Mv3ModelEntity {
    fn on_loading(&mut self, entity: &mut CoreEntity<Mv3ModelEntity>) {
        entity.add_component(RenderObject::new_host_dynamic_with_data(
            self.vertices[0].clone(),
            self.indices.clone(),
            Box::new(SimpleMaterial::new(&self.texture_path)),
        ));
    }

    fn on_updating(&mut self, entity: &mut CoreEntity<Mv3ModelEntity>, delta_sec: f32) {
        let mut anim_time = (delta_sec * 4580.) as u32 + self.last_anim_time;
        let total_anim_length = *self.anim_timestamps.last().unwrap();

        if anim_time >= total_anim_length && self.repeat_mode == Mv3AnimRepeatMode::NoRepeat {
            self.anim_finished = true;
            return;
        }

        anim_time %= total_anim_length;
        let frame_index = self
            .anim_timestamps
            .iter()
            .position(|&t| t > anim_time)
            .unwrap_or(0)
            - 1;
        let next_frame_index = (frame_index + 1) % self.anim_timestamps.len();
        let percentile = (anim_time - self.anim_timestamps[frame_index]) as f32
            / (self.anim_timestamps[next_frame_index] - self.anim_timestamps[frame_index]) as f32;

        entity
            .get_component_mut::<RenderObject>()
            .unwrap()
            .update_vertices(&|vertices: &mut VertexBuffer| {
                let vertex_buffer = self.vertices.get(frame_index).unwrap();
                let next_vertex_buffer = self.vertices.get(next_frame_index).unwrap();

                for i in 0..self.vertices.get(frame_index).unwrap().count() {
                    let position = vertex_buffer.position(i).unwrap();
                    let next_position = next_vertex_buffer.position(i).unwrap();
                    let tex_coord = vertex_buffer.tex_coord(i).unwrap();

                    vertices.set_component(i, VertexComponents::POSITION, |p: &mut Vec3| {
                        p.x = position.x * (1. - percentile) + next_position.x * percentile;
                        p.y = position.y * (1. - percentile) + next_position.y * percentile;
                        p.z = position.z * (1. - percentile) + next_position.z * percentile;
                    });
                    vertices.set_component(i, VertexComponents::TEXCOORD, |t: &mut Vec2| {
                        t.x = tex_coord.x;
                        t.y = tex_coord.y;
                    });
                }
            });

        self.last_anim_time = anim_time;
    }
}
