use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use crosscom::ComRc;
use serde::Serialize;

use crate::{
    comdef::{
        IArmatureComponent, IArmatureComponentImpl, IComponentImpl, IEntity, IHAnimBoneComponent,
        IHAnimBoneComponentImpl, ISkinnedMeshComponentImpl,
    },
    math::{Mat44, Quaternion, Transform, Vec3},
    rendering::{ComponentFactory, VertexBuffer, VertexComponents},
    ComObject_ArmatureComponent, ComObject_HAnimBoneComponent, ComObject_SkinnedMeshComponent,
};

use super::Geometry;

pub struct SkinnedMeshComponent {
    entity: ComRc<IEntity>,
    component_factory: Rc<dyn ComponentFactory>,
    geometry: Geometry,
    armature: ComRc<IArmatureComponent>,
    bones: Vec<ComRc<IEntity>>,
    bone_components: Vec<ComRc<IHAnimBoneComponent>>,
    v_bone_id: Vec<[usize; 4]>,
    v_weights: Vec<[f32; 4]>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AnimationState {
    NoAnimation,
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct AnimKeyFrame {
    pub rotation: Quaternion,
    pub position: Vec3,
    pub timestamp: f32,
}

impl SkinnedMeshComponent {
    pub fn new(
        entity: ComRc<IEntity>,
        component_factory: Rc<dyn ComponentFactory>,
        geometry: Geometry,
        armature: ComRc<IArmatureComponent>,
        v_bone_id: Vec<[usize; 4]>,
        v_weights: Vec<[f32; 4]>,
    ) -> Self {
        let bones = armature.bones();
        let bone_components = bones
            .iter()
            .map(|b| {
                b.get_component(IHAnimBoneComponent::uuid())
                    .unwrap()
                    .query_interface::<IHAnimBoneComponent>()
                    .unwrap()
            })
            .collect();

        Self {
            entity,
            component_factory,
            geometry,
            armature,
            bones,
            bone_components,
            v_bone_id,
            v_weights,
        }
    }

    fn load_geometries(&self) {
        let mut objects = vec![];

        let ro = self.component_factory.create_render_object(
            self.geometry.vertices.clone(),
            self.geometry.indices.clone(),
            &self.geometry.material,
            true,
        );

        objects.push(ro);

        let component = self.component_factory.create_rendering_component(objects);
        self.entity
            .set_rendering_component(Some(Rc::new(component)));
    }

    fn update_vertex_buffer(&self, mut vertex_buffer: RefMut<VertexBuffer>) {
        let use_bond_pose = self.armature.animation_state() == AnimationState::NoAnimation;
        for i in 0..vertex_buffer.count() {
            let bone_id = self.v_bone_id[i][0];
            let bond_pose_mat = self.bone_components[bone_id].bond_pose();
            let v = self.geometry.vertices.position(i).unwrap();

            let v = if use_bond_pose {
                *v
            } else {
                Vec3::crossed_mat(
                    &Vec3::crossed_mat(v, &bond_pose_mat),
                    self.bones[bone_id].world_transform().matrix(),
                )
            };

            vertex_buffer.set_component(i, VertexComponents::POSITION, |p: &mut Vec3| {
                *p = v;
            });
        }
    }
}

ComObject_SkinnedMeshComponent!(super::SkinnedMeshComponent);

impl ISkinnedMeshComponentImpl for SkinnedMeshComponent {}

impl IComponentImpl for SkinnedMeshComponent {
    fn on_loading(&self) {
        self.load_geometries();
    }

    fn on_updating(&self, delta_sec: f32) {
        let rc = self.entity.get_rendering_component().unwrap();
        let objects = rc.render_objects();

        if objects.len() > 0 {
            let ro = &objects[0];
            ro.update_vertices(&|vb: RefMut<VertexBuffer>| {
                self.update_vertex_buffer(vb);
            });
        }
    }
}

pub struct ArmatureComponent {
    entity: ComRc<IEntity>,
    root_bone: ComRc<IEntity>,
    bones: Vec<ComRc<IEntity>>,
    animation_state: RefCell<AnimationState>,
}

ComObject_ArmatureComponent!(super::ArmatureComponent);

impl ArmatureComponent {
    pub fn new(
        entity: ComRc<IEntity>,
        root_bone: ComRc<IEntity>,
        bones: Vec<ComRc<IEntity>>,
    ) -> Self {
        Self {
            entity,
            root_bone,
            bones,
            animation_state: RefCell::new(AnimationState::NoAnimation),
        }
    }
}

impl IArmatureComponentImpl for ArmatureComponent {
    fn set_animation(&self, keyframes: Vec<Vec<AnimKeyFrame>>) {
        for b in self.bones.iter().zip(keyframes) {
            b.0.get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap()
                .set_keyframes(b.1);
        }

        self.animation_state.replace(AnimationState::Playing);
    }

    fn clear_animation(&self) {
        self.animation_state.replace(AnimationState::NoAnimation);
        for b in &self.bones {
            b.get_component(IHAnimBoneComponent::uuid())
                .unwrap()
                .query_interface::<IHAnimBoneComponent>()
                .unwrap()
                .set_keyframes(vec![]);
        }
    }

    fn animation_state(&self) -> AnimationState {
        *self.animation_state.borrow()
    }

    fn bones(&self) -> Vec<ComRc<IEntity>> {
        self.bones.clone()
    }
}

impl IComponentImpl for ArmatureComponent {
    fn on_loading(&self) {}

    fn on_updating(&self, delta_sec: f32) {
        self.root_bone.update(delta_sec);
        self.root_bone.update_world_transform(&Transform::new());
    }
}

pub struct HAnimBoneComponent {
    entity: ComRc<IEntity>,
    id: u32,
    props: RefCell<HAnimBoneProps>,
}

ComObject_HAnimBoneComponent!(super::HAnimBoneComponent);

struct HAnimBoneProps {
    bond_pose: Mat44,
    frames: Vec<AnimKeyFrame>,
    last_time: f32,
    max_time: f32,
}

impl HAnimBoneProps {
    pub fn update(&mut self, entity: ComRc<IEntity>, delta_sec: f32) {
        self.last_time = self.last_time + delta_sec;

        if self.last_time > self.max_time {
            self.last_time = 0.;
        }

        let frame_index = self
            .frames
            .iter()
            .position(|t| t.timestamp > self.last_time)
            .unwrap_or(0);

        let next_frame_index = (frame_index + 1).min(self.frames.len() - 1);
        let pct = if frame_index == next_frame_index {
            0.
        } else {
            (self.last_time - self.frames[frame_index].timestamp)
                / (self.frames[next_frame_index].timestamp)
        };

        let rotation = Quaternion::slerp(
            &self.frames[frame_index].rotation,
            &self.frames[next_frame_index].rotation,
            pct,
        );

        let position = Vec3::lerp(
            &self.frames[frame_index].position,
            &self.frames[next_frame_index].position,
            pct,
        );

        let mut frame_mat = rotation.to_rotate_matrix();
        frame_mat[0][3] = position.x;
        frame_mat[1][3] = position.y;
        frame_mat[2][3] = position.z;

        let b = entity.transform();
        b.borrow_mut().set_matrix(frame_mat);
    }
}

impl HAnimBoneComponent {
    pub fn new(entity: ComRc<IEntity>, id: u32) -> Self {
        Self {
            entity,
            id,
            props: RefCell::new(HAnimBoneProps {
                bond_pose: Mat44::new_identity(),
                frames: vec![],
                last_time: 0.,
                max_time: 0.,
            }),
        }
    }
}

impl IHAnimBoneComponentImpl for HAnimBoneComponent {
    fn set_keyframes(&self, keyframes: Vec<AnimKeyFrame>) {
        self.props.borrow_mut().max_time = keyframes.last().unwrap().timestamp;
        self.props.borrow_mut().frames = keyframes;
    }

    fn set_bond_pose(&self, matrix: Mat44) {
        self.props.borrow_mut().bond_pose = matrix;
    }

    fn bond_pose(&self) -> Mat44 {
        self.props.borrow().bond_pose.clone()
    }
}

impl IComponentImpl for HAnimBoneComponent {
    fn on_loading(&self) {}

    fn on_updating(&self, delta_sec: f32) {
        if self.props.borrow().frames.is_empty() {
            return;
        }

        self.props
            .borrow_mut()
            .update(self.entity.clone(), delta_sec);
    }
}
