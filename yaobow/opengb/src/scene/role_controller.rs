use crate::asset_manager::AssetManager;
use crate::comdef::{IRoleController, IRoleControllerImpl};
use crate::ComObject_RoleController;
use common::store_ext::StoreExt2;
use crosscom::ComRc;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use fileformats::mv3::{read_mv3, Mv3Model};
use mini_fs::{MiniFs, StoreExt};
use radiance::comdef::{IAnimatedMeshComponent, IComponent, IComponentImpl, IEntity};
use radiance::math::Vec3;
use radiance::rendering::{
    AnimatedMeshComponent, ComponentFactory, Geometry, MaterialDef, MorphAnimationState,
    MorphTarget, SimpleMaterialDef, TexCoord,
};
use radiance::scene::CoreEntity;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
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
    AnimationFinished,
    Idle,
    Walking,
    Running,
}

pub fn create_mv3_entity(
    asset_mgr: Rc<AssetManager>,
    role_name: &str,
    idle_anim: &str,
    name: String,
    visible: bool,
) -> Result<ComRc<IEntity>, EntityError> {
    let entity = CoreEntity::create(name, visible);

    entity.add_component(
        IRoleController::uuid(),
        crosscom::ComRc::from_object(RoleController::new(
            entity.clone(),
            asset_mgr,
            role_name,
            idle_anim,
        )?),
    );

    Ok(entity)
}

pub fn create_mv3_entity_from_animation(
    asset_mgr: Rc<AssetManager>,
    role_name: &str,
    idle_anim_name: &str,
    idle_anim: ComRc<IAnimatedMeshComponent>,
    name: String,
    visible: bool,
) -> Result<ComRc<IEntity>, EntityError> {
    let entity = CoreEntity::create(name, visible);
    entity.add_component(
        IRoleController::uuid(),
        crosscom::ComRc::from_object(RoleController::new_from_idle_animation(
            entity.clone(),
            asset_mgr,
            role_name,
            idle_anim_name,
            idle_anim,
        )),
    );

    Ok(entity)
}

pub struct RoleController {
    entity: ComRc<IEntity>,
    model_name: String,
    asset_mgr: Rc<AssetManager>,
    component_factory: Rc<dyn ComponentFactory>,
    animations: DashMap<String, ComRc<IAnimatedMeshComponent>>,
    active_anim_name: RefCell<String>,
    idle_anim_name: String,
    walking_anim_name: String,
    running_anim_name: String,
    anim_repeat_mode: RefCell<RoleAnimationRepeatMode>,
    is_active: RefCell<bool>,
    state: RefCell<RoleState>,
    auto_play_idle: RefCell<bool>,
    nav_layer: RefCell<usize>,
    proc_id: RefCell<i32>,
}

ComObject_RoleController!(super::RoleController);

impl RoleController {
    pub fn new(
        entity: ComRc<IEntity>,
        asset_mgr: Rc<AssetManager>,
        role_name: &str,
        idle_anim: &str,
    ) -> Result<Self, EntityError> {
        let idle_anim = idle_anim;
        let anim = asset_mgr
            .load_role_anim_first(entity.clone(), role_name, &[idle_anim, "c01", "z1"])
            .ok_or(EntityError::EntityAnimationNotFound)?;

        Ok(Self::new_from_idle_animation(
            entity, asset_mgr, role_name, anim.0, anim.1,
        ))
    }

    pub fn new_from_idle_animation(
        entity: ComRc<IEntity>,
        asset_mgr: Rc<AssetManager>,
        role_name: &str,
        idle_anim_name: &str,
        idle_anim: ComRc<IAnimatedMeshComponent>,
    ) -> Self {
        let animations = DashMap::new();
        if !idle_anim_name.trim().is_empty() {
            animations.insert(idle_anim_name.to_string(), idle_anim);
        }

        let walking_anim =
            asset_mgr.load_role_anim_first(entity.clone(), role_name, &["c02", "z3"]);
        let running_anim =
            asset_mgr.load_role_anim_first(entity.clone(), role_name, &["c03", "c02", "z3"]);

        let walking_anim_name = walking_anim
            .map(|(name, _)| name)
            .unwrap_or(idle_anim_name)
            .to_string();

        let running_anim_name = running_anim
            .map(|(name, _)| name)
            .unwrap_or(walking_anim_name.as_str())
            .to_string();

        Self {
            entity,
            model_name: role_name.to_string(),
            asset_mgr: asset_mgr.clone(),
            component_factory: asset_mgr.component_factory().clone(),
            animations,
            active_anim_name: RefCell::new(idle_anim_name.to_string()),
            idle_anim_name: idle_anim_name.to_string(),
            walking_anim_name,
            running_anim_name,
            anim_repeat_mode: RefCell::new(RoleAnimationRepeatMode::Repeat),
            is_active: RefCell::new(false),
            state: RefCell::new(RoleState::Idle),
            auto_play_idle: RefCell::new(true),
            nav_layer: RefCell::new(0),
            proc_id: RefCell::new(0),
        }
    }

    pub fn try_get_role_model(entity: ComRc<IEntity>) -> Option<ComRc<IRoleController>> {
        entity
            .get_component(IRoleController::uuid())?
            .query_interface::<IRoleController>()
    }

    pub fn is_active(&self) -> bool {
        *self.is_active.borrow()
    }

    pub fn set_active(&self, active: bool) {
        *self.is_active.borrow_mut() = active;
        if active {
            let anim_name = { self.active_anim_name.borrow().clone() };
            let mode = *self.anim_repeat_mode.borrow();
            self.play_anim(&anim_name, mode);
        } else {
            self.entity.set_rendering_component(None);
        }

        self.entity.set_visible(active);
    }

    pub fn proc_id(&self) -> i32 {
        *self.proc_id.borrow()
    }

    pub fn set_proc_id(&self, proc_id: i32) {
        *self.proc_id.borrow_mut() = proc_id;
    }

    pub fn play_anim(&self, anim_name: &str, repeat_mode: RoleAnimationRepeatMode) {
        let anim_name = if anim_name.is_empty() {
            self.idle_anim_name.to_lowercase()
        } else {
            let mut anim_name = anim_name.to_lowercase();
            if self.animations.get(&anim_name).is_none() {
                let anim = self.asset_mgr.load_role_anim(
                    self.entity.clone(),
                    &self.model_name,
                    &anim_name,
                );
                if let Some(anim) = anim {
                    self.animations.insert(anim_name.to_string(), anim);
                } else {
                    anim_name = self.idle_anim_name.to_lowercase();
                }
            }

            anim_name
        };

        *self.active_anim_name.borrow_mut() = anim_name.to_string();
        *self.anim_repeat_mode.borrow_mut() = repeat_mode;
        *self.state.borrow_mut() = RoleState::PlayingAnimation;

        self.entity.add_component(
            IAnimatedMeshComponent::uuid(),
            self.active_anim()
                .value()
                .query_interface::<IComponent>()
                .unwrap(),
        );
    }

    pub fn run(&self) {
        if *self.state.borrow() != RoleState::Running {
            let name = self.running_anim_name.clone();
            self.play_anim(&name, RoleAnimationRepeatMode::Repeat);
            *self.state.borrow_mut() = RoleState::Running;
        }
    }

    pub fn idle(&self) {
        if *self.state.borrow() != RoleState::Idle {
            let name = self.idle_anim_name.clone();
            self.play_anim(&name, RoleAnimationRepeatMode::Repeat);
            *self.state.borrow_mut() = RoleState::Idle;
        }
    }

    pub fn walk(&self) {
        if *self.state.borrow() != RoleState::Walking {
            let name = self.walking_anim_name.clone();
            self.play_anim(&name, RoleAnimationRepeatMode::Repeat);
            *self.state.borrow_mut() = RoleState::Walking;
        }
    }

    pub fn set_auto_play_idle(&self, auto_play_idle: bool) {
        *self.auto_play_idle.borrow_mut() = auto_play_idle;
    }

    pub fn state(&self) -> RoleState {
        *self.state.borrow()
    }

    pub fn nav_layer(&self) -> usize {
        *self.nav_layer.borrow()
    }

    pub fn switch_nav_layer(&self) -> usize {
        *self.nav_layer.borrow_mut() = (self.nav_layer() + 1) % 2;
        self.nav_layer()
    }

    pub fn set_nav_layer(&self, layer: usize) {
        *self.nav_layer.borrow_mut() = layer;
    }

    fn active_anim(&self) -> Ref<String, ComRc<IAnimatedMeshComponent>> {
        self.animations
            .get(&*self.active_anim_name.borrow())
            .unwrap()
    }
}

impl IRoleControllerImpl for RoleController {
    fn get(&self) -> &'static crate::scene::RoleController {
        unsafe { &*(self as *const _) }
    }
}

impl IComponentImpl for RoleController {
    fn on_loading(&self) -> crosscom::Void {
        if !self.idle_anim_name.trim().is_empty() && self.is_active() {
            self.idle();
        }
    }

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {
        if self.is_active() {
            if self.active_anim().value().morph_animation_state() == MorphAnimationState::Finished {
                self.state.replace(RoleState::AnimationFinished);
                if *self.anim_repeat_mode.borrow() == RoleAnimationRepeatMode::NoRepeat {
                    if *self.auto_play_idle.borrow() {
                        self.idle();
                    }
                } else {
                    self.active_anim().value().replay();
                }
            }
        }
    }
}

pub fn create_animated_mesh_from_mv3<P: AsRef<Path>>(
    entity: ComRc<IEntity>,
    component_factory: &Rc<dyn ComponentFactory>,
    vfs: &MiniFs,
    path: P,
) -> anyhow::Result<ComRc<IAnimatedMeshComponent>> {
    let mv3file = read_mv3(&mut Cursor::new(vfs.read_to_end(&path)?))?;
    let mut frames = vec![];

    for model_index in 0..mv3file.models.len() {
        let model = &mv3file.models[model_index];
        let mut texture_path = path.as_ref().to_owned();
        texture_path.pop();
        let texture_index = if model_index < mv3file.texture_count as usize {
            model_index
        } else {
            0
        };

        texture_path.push(std::str::from_utf8(&mv3file.textures[texture_index].names[0]).unwrap());

        let material = SimpleMaterialDef::create(
            texture_path.to_str().unwrap(),
            |name| vfs.open(name).ok(),
            false,
        );
        for mesh_index in 0..model.mesh_count as usize {
            frames.push(create_geometry_frames(model, mesh_index, &material))
        }
    }

    let anim_timestamps: Vec<u32> = mv3file.models[0]
        .frames
        .iter()
        .map(|f| f.timestamp)
        .collect();

    let mut morph_targets: Vec<MorphTarget> = Vec::<MorphTarget>::with_capacity(frames.len());
    for frame_index in 0..mv3file.models[0].frame_count as usize {
        let mut geometries = vec![];
        for mesh_index in 0..frames.len() {
            geometries.push(frames[mesh_index][frame_index].clone());
        }

        morph_targets.push(MorphTarget::new(
            geometries,
            anim_timestamps[frame_index] as f32 / 4580.,
            component_factory.clone(),
        ));
    }

    let animated_mesh = AnimatedMeshComponent::new(entity, component_factory.clone());
    animated_mesh.set_morph_targets(morph_targets);

    Ok(ComRc::from_object(animated_mesh))
}

fn create_geometry_frames(
    model: &Mv3Model,
    mesh_index: usize,
    material: &MaterialDef,
) -> Vec<Geometry> {
    let mesh = &model.meshes[mesh_index];
    let hash = |index, texcoord_index| index as u32 * model.texcoord_count + texcoord_index as u32;

    let mut indices: Vec<u32> = Vec::<u32>::with_capacity(model.vertex_per_frame as usize);
    let mut vertices = vec![vec![]; model.frame_count as usize];
    let mut texcoord = vec![vec![]; model.frame_count as usize];
    let mut index_map = HashMap::new();

    for t in &mesh.triangles {
        for (&i, &j) in t.indices.iter().zip(&t.texcoord_indices) {
            let h = hash(i, j);
            let index = match index_map.get(&h) {
                None => {
                    let index = index_map.len();
                    for k in 0..model.frame_count as usize {
                        let frame = &model.frames[k];
                        vertices[k].push(Vec3::new(
                            frame.vertices[i as usize].x as f32 * 0.01562,
                            frame.vertices[i as usize].y as f32 * 0.01562,
                            frame.vertices[i as usize].z as f32 * 0.01562,
                        ));

                        if (j as u32) < model.texcoord_count {
                            texcoord[k].push(TexCoord::new(
                                model.texcoords[j as usize].u,
                                -model.texcoords[j as usize].v,
                            ));
                        } else {
                            texcoord[k].push(TexCoord::new(0., 0.));
                        }
                    }
                    index_map.insert(h, index as u32);
                    index as u32
                }
                Some(index) => *index,
            };

            indices.push(index);
        }
    }

    let mut geometries = vec![];
    for i in 0..model.frame_count as usize {
        geometries.push(Geometry::new(
            &vertices[i],
            None,
            &vec![texcoord[i].clone()],
            indices.clone(),
            material.clone(),
            1,
        ))
    }

    geometries
}
