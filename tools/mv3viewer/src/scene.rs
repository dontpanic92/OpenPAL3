use opengb::loaders::mv3loader::*;
use radiance::scene::{CoreScene, SceneCallbacks, Entity, CoreEntity, EntityCallbacks};
use radiance::rendering::{RenderObject, Vertex};
use radiance::math::{Vec2, Vec3};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ModelEntity {
    texture_path: PathBuf,
    vertices: Vec<Vec<Vertex>>,
    indices: Vec<u32>,
    anim_timestamps: Vec<u32>,
    last_anim_time: u32,
    mv3: Mv3File,
}

impl ModelEntity {
    pub fn new(path: &String) -> Self {
        let mv3file = mv3_load_from_file(&path).unwrap();
        let model: &Mv3Model = &mv3file.models[0];
        let mesh: &Mv3Mesh = &model.meshes[0];

        let mut texture_path = PathBuf::from(path);
        texture_path.pop();
        texture_path.push(std::str::from_utf8(&mv3file.textures[0].names[0]).unwrap());

        let hash = |index, texcoord_index| {
            index as u32 * model.texcoord_count + texcoord_index as u32
        };

        let mut vertices: Vec<Vec<Vertex>> = Vec::<Vec<Vertex>>::with_capacity(model.frame_count as usize);
        for _i in 0..model.frame_count {
            vertices.push(Vec::<Vertex>::with_capacity(std::cmp::max(model.texcoord_count, model.vertex_per_frame) as usize));
        }

        let mut indices: Vec<u32> = Vec::<u32>::with_capacity(model.vertex_per_frame as usize);
        let mut m = HashMap::new();
        for t in &mesh.triangles {
            for (&i, &j) in t.indices.iter().zip(&t.texcoord_indices) {
                let h = hash(i, j);
                let index = match m.get(&h) {
                    None => {
                        let index = m.len();
                        for k in 0..model.frame_count as usize {
                            let frame = &model.frames[k];
                            vertices.get_mut(k).unwrap().push(Vertex::new(
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

                        m.insert(h, index as u32);
                        index as u32
                    },
                    Some(index) => *index,
                };

                indices.push(index);
            }
        }

        let anim_timestamps = model.frames.iter().map(|f| f.timestamp).collect();

        ModelEntity {
            texture_path,
            anim_timestamps,
            last_anim_time: 0,
            vertices,
            indices,
            mv3: mv3file,
        }
    }
}

impl EntityCallbacks for ModelEntity {
    fn on_loading<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>) {
        entity.add_component(RenderObject::new_with_data(self.vertices[0].clone(), self.indices.clone(), &self.texture_path));
    }

    fn on_updating<T: EntityCallbacks>(&mut self, entity: &mut CoreEntity<T>, delta_sec: f32) {
        entity.transform_mut().rotate_local(&Vec3::new(0., 1., 0.), -0.2 * delta_sec * std::f32::consts::PI);

        let anim_time = ((delta_sec * 4580.) as u32 + self.last_anim_time) % self.anim_timestamps.last().unwrap();

        let frame_index = self.anim_timestamps.iter().position(|&t| t > anim_time).unwrap_or(0) - 1;
        let next_frame_index = (frame_index + 1) % self.anim_timestamps.len();
        let percentile = (anim_time - self.anim_timestamps[frame_index]) as f32 / (self.anim_timestamps[next_frame_index] - self.anim_timestamps[frame_index]) as f32;

        entity.get_component_mut::<RenderObject>().unwrap().update_vertices(&|vertices: &mut Vec<Vertex>| {
            for (i, (vert, next_vert)) in self.vertices[frame_index].iter().zip(&self.vertices[next_frame_index]).enumerate() {
                let position = vert.position();
                let next_position = next_vert.position();

                let v = vertices.get_mut(i).unwrap();
                v.position_mut().x = position.x * (1. - percentile) + next_position.x * percentile;
                v.position_mut().y = position.y * (1. - percentile) + next_position.y * percentile;
                v.position_mut().z = position.z * (1. - percentile) + next_position.z * percentile;
                v.tex_coord_mut().x = vert.tex_coord().x;
                v.tex_coord_mut().y = vert.tex_coord().y;
            }
        });

        self.last_anim_time = anim_time;
    }
}

pub struct ModelViewerScene {
    pub path: String,
}

impl SceneCallbacks for ModelViewerScene {
    fn on_loading<T: SceneCallbacks>(&mut self, scene: &mut CoreScene<T>) {
        let mut entity = CoreEntity::new(ModelEntity::new(&self.path));
        entity.transform_mut().translate(&Vec3::new(0., -40., -100.));
        scene.add_entity(entity);
    }
}
