use crate::{asset_manager::AssetManager, loaders::mv3_loader::*};
use radiance::{rendering::{ComponentFactory, MaterialDef, VertexBuffer, VertexComponents}, scene::Entity};
use radiance::scene::{CoreEntity, EntityExtension};
use radiance::{
    math::{Vec2, Vec3},
    rendering::RenderingComponent,
};
use std::collections::HashMap;
use std::rc::Rc;

use super::error::EntityError;

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum RoleAnimationRepeatMode {
    NoRepeat,
    Repeat,
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum RoleState {
    PlayingAnimation,
    Idle,
    Walking,
    Running,
}

pub struct RoleEntity {
    model_name: String,
    asset_mgr: Rc<AssetManager>,
    component_factory: Rc<dyn ComponentFactory>,
    animations: HashMap<String, RoleAnimation>,
    active_anim_name: String,
    idle_anim_name: String,
    walking_anim_name: String,
    anim_repeat_mode: RoleAnimationRepeatMode,
    is_active: bool,
    state: RoleState,
    auto_play_idle: bool,
}

impl RoleEntity {
    pub fn new(
        asset_mgr: Rc<AssetManager>,
        role_name: &str,
        idle_anim: &str,
    ) -> Result<RoleEntity, EntityError> {
        let mut idle_anim = idle_anim;
        let anim = asset_mgr
            .load_role_anim_first(role_name, &[idle_anim, "c01", "z1"])
            .ok_or(EntityError::EntityAnimationNotFound)?;

        Ok(Self::new_from_idle_animation(
            asset_mgr, role_name, anim.0, anim.1,
        ))
    }

    pub fn new_from_idle_animation(
        asset_mgr: Rc<AssetManager>,
        role_name: &str,
        idle_anim_name: &str,
        idle_anim: RoleAnimation,
    ) -> RoleEntity {
        let mut animations = HashMap::new();
        if !idle_anim_name.trim().is_empty() {
            animations.insert(idle_anim_name.to_string(), idle_anim);
        }

        let walking_anim = asset_mgr.load_role_anim_first(role_name, &["c02", "z3"]);

        let walking_anim_name = walking_anim
            .map(|(name, _)| name)
            .unwrap_or(idle_anim_name)
            .to_string();

        Self {
            model_name: role_name.to_string(),
            asset_mgr: asset_mgr.clone(),
            component_factory: asset_mgr.component_factory().clone(),
            animations,
            active_anim_name: idle_anim_name.to_string(),
            idle_anim_name: idle_anim_name.to_string(),
            walking_anim_name,
            anim_repeat_mode: RoleAnimationRepeatMode::Repeat,
            is_active: false,
            state: RoleState::Idle,
            auto_play_idle: true,
        }
    }

    pub fn set_active(self: &mut CoreEntity<Self>, active: bool) {
        self.is_active = active;
        if active {
            let anim_name = self.active_anim_name.clone();
            self.play_anim(&anim_name, self.anim_repeat_mode);
        } else {
            self.remove_component::<RenderingComponent>();
        }
    }

    pub fn play_anim(
        self: &mut CoreEntity<Self>,
        anim_name: &str,
        repeat_mode: RoleAnimationRepeatMode,
    ) {
        let anim_name = if anim_name.is_empty() {
            self.idle_anim_name.to_lowercase()
        } else {
            anim_name.to_lowercase()
        };

        self.state = if anim_name == self.idle_anim_name.to_lowercase() {
            RoleState::Idle
        } else if anim_name == self.walking_anim_name.to_lowercase() {
            RoleState::Walking
        } else if anim_name == "c03" {
            RoleState::Running
        } else {
            RoleState::PlayingAnimation
        };

        if self.animations.get(&anim_name).is_none() {
            let anim = self
                .asset_mgr
                .load_role_anim(&self.model_name, &anim_name)
                .unwrap();
            self.animations.insert(anim_name.to_string(), anim);
        }

        self.active_anim_name = anim_name.to_string();
        self.anim_repeat_mode = repeat_mode;
        self.active_anim_mut().reset(repeat_mode);

        self.remove_component::<RenderingComponent>();
        let rc = self.active_anim().create_rendering_component();
        self.add_component(Box::new(rc));
    }

    pub fn run(self: &mut CoreEntity<Self>) {
        if self.state != RoleState::Running {
            self.play_anim("c03", RoleAnimationRepeatMode::Repeat);
        }
    }

    pub fn idle(self: &mut CoreEntity<Self>) {
        if self.state != RoleState::Idle {
            let name = self.idle_anim_name.clone();
            self.play_anim(&name, RoleAnimationRepeatMode::Repeat);
        }
    }

    pub fn walk(self: &mut CoreEntity<Self>) {
        if self.state != RoleState::Walking {
            let name = self.walking_anim_name.clone();
            self.play_anim(&name, RoleAnimationRepeatMode::Repeat);
        }
    }

    pub fn set_auto_play_idle(&mut self, auto_play_idle: bool) {
        self.auto_play_idle = auto_play_idle;
    }

    pub fn state(&self) -> RoleState {
        self.state
    }

    fn active_anim(&self) -> &RoleAnimation {
        self.animations.get(&self.active_anim_name).unwrap()
    }

    fn active_anim_mut(&mut self) -> &mut RoleAnimation {
        self.animations.get_mut(&self.active_anim_name).unwrap()
    }
}

impl EntityExtension for RoleEntity {
    fn on_loading(self: &mut CoreEntity<Self>) {
        if !self.idle_anim_name.trim().is_empty() && self.is_active {
            let name = self.idle_anim_name.clone();
            self.play_anim(&name, RoleAnimationRepeatMode::Repeat);
        }
    }

    fn on_updating(self: &mut CoreEntity<Self>, delta_sec: f32) {
        if self.is_active {
            // TODO: Consider to use Arc<Mutex<>>>
            let rc = unsafe {
                &mut *(self.get_component_mut::<RenderingComponent>().unwrap()
                    as *mut RenderingComponent)
            };
            let ro = rc.render_objects_mut().first_mut().unwrap();

            ro.update_vertices(&mut |vb: &mut VertexBuffer| {
                self.active_anim_mut().update(delta_sec, vb, false);
            });

            if self.active_anim().anim_finished() {
                self.state = RoleState::Idle;

                if self.state == RoleState::Idle && self.auto_play_idle {
                    self.play_anim("", RoleAnimationRepeatMode::Repeat);
                }
            }
        }
    }
}

pub struct RoleAnimation {
    component_factory: Rc<dyn ComponentFactory>,
    frames: Vec<VertexBuffer>,
    anim_timestamps: Vec<u32>,
    last_anim_time: u32,
    repeat_mode: RoleAnimationRepeatMode,
    anim_finished: bool,
    vertices: VertexBuffer,
    indices: Vec<u32>,
    material: MaterialDef,
}

impl RoleAnimation {
    pub fn new(
        component_factory: &Rc<dyn ComponentFactory>,
        mv3file: &Mv3File,
        material: MaterialDef,
        anim_repeat_mode: RoleAnimationRepeatMode,
    ) -> Self {
        let model: &Mv3Model = &mv3file.models[0];
        let mesh: &Mv3Mesh = &model.meshes[0];

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

        let mut frames: Vec<VertexBuffer> =
            Vec::<VertexBuffer>::with_capacity(model.frame_count as usize);
        for i in 0..model.frame_count as usize {
            frames.push(VertexBuffer::new(
                VertexComponents::POSITION | VertexComponents::TEXCOORD,
                index_map.len(),
            ));

            let vertex_data = &vertices_data[i];
            let vert = frames.get_mut(i).unwrap();
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
        let vertices = frames[0].clone();

        Self {
            component_factory: component_factory.clone(),
            frames,
            anim_timestamps,
            last_anim_time: 0,
            repeat_mode: anim_repeat_mode,
            anim_finished: false,
            vertices,
            indices,
            material,
        }
    }

    pub fn reset(&mut self, repeat_mode: RoleAnimationRepeatMode) {
        self.anim_finished = false;
        self.last_anim_time = 0;
        self.repeat_mode = repeat_mode;
    }

    pub fn update(&mut self, delta_sec: f32, vertices: &mut VertexBuffer, debug: bool) {
        let mut anim_time = (delta_sec * 4580.) as u32 + self.last_anim_time;
        let total_anim_length = *self.anim_timestamps.last().unwrap();
        if anim_time >= total_anim_length && self.repeat_mode == RoleAnimationRepeatMode::NoRepeat {
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
        if debug {
            println!("frame_index {}", frame_index);
        }
        let next_frame_index = (frame_index + 1) % self.anim_timestamps.len();
        let percentile = (anim_time - self.anim_timestamps[frame_index]) as f32
            / (self.anim_timestamps[next_frame_index] - self.anim_timestamps[frame_index]) as f32;

        let vertex_buffer = self.frames.get(frame_index).unwrap();
        let next_vertex_buffer = self.frames.get(next_frame_index).unwrap();

        for i in 0..self.frames.get(frame_index).unwrap().count() {
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

        self.last_anim_time = anim_time;
    }

    pub fn anim_finished(&self) -> bool {
        self.anim_finished
    }

    pub fn create_rendering_component(&self) -> RenderingComponent {
        let ro = self.component_factory.create_render_object(
            self.vertices.clone(),
            self.indices.clone(),
            &self.material,
            true,
        );

        self.component_factory.create_rendering_component(vec![ro])
    }
}
