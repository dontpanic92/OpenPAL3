use std::{
    borrow::Borrow,
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use crosscom::ComRc;
use serde::Serialize;

use crate::{
    comdef::{IComponentImpl, IEntity, ISkinnedMeshComponentImpl},
    math::{Mat44, Quaternion, Transform, Vec3},
    rendering::{ComponentFactory, VertexBuffer, VertexComponents},
    ComObject_SkinnedMeshComponent,
};

use super::Geometry;

pub struct SkinnedMeshComponent {
    entity: ComRc<IEntity>,
    component_factory: Rc<dyn ComponentFactory>,
    geometry: Geometry,
    bones: Vec<ComRc<IEntity>>,
    bond_pose: Vec<Transform>,
    v_bone_id: Vec<[usize; 4]>,
    v_weights: Vec<[f32; 4]>,
    props: RefCell<Props>,
}

#[derive(Debug, Copy, Clone, Serialize)]
pub struct AnimKeyFrame {
    pub rotation: Quaternion,
    pub position: Vec3,
    pub timestamp: f32,
}

struct Props {
    frames: Option<Vec<Vec<AnimKeyFrame>>>,
    last_time: f32,
    max_time: f32,
}

impl SkinnedMeshComponent {
    pub fn new(
        entity: ComRc<IEntity>,
        component_factory: Rc<dyn ComponentFactory>,
        geometry: Geometry,
        bones: Vec<ComRc<IEntity>>,
        v_bone_id: Vec<[usize; 4]>,
        v_weights: Vec<[f32; 4]>,
    ) -> Self {
        let bond_pose = bones
            .iter()
            .map(|b| b.transform().as_ref().borrow().clone())
            .collect();
        Self {
            entity,
            component_factory,
            geometry,
            bones,
            bond_pose,
            v_bone_id,
            v_weights,
            props: RefCell::new(Props {
                frames: None,
                last_time: 0.,
                max_time: 0.,
            }),
        }
    }

    fn props(&self) -> Ref<Props> {
        self.props.borrow()
    }

    fn props_mut(&self) -> RefMut<Props> {
        self.props.borrow_mut()
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

    pub fn set_keyframes(&self, keyframes: Vec<Vec<AnimKeyFrame>>) {
        self.props.borrow_mut().max_time = keyframes[0].last().unwrap().timestamp;
        self.props.borrow_mut().frames = Some(keyframes);
    }

    fn update_vertex_buffer(&self, anim_timestamp: f32, mut vertex_buffer: RefMut<VertexBuffer>) {
        let props = self.props();
        if props.frames.is_none() {
            return;
        }

        for i in 0..vertex_buffer.count() {
            let bone_id = self.v_bone_id[i][0];

            let bond_pose_mat = *self.bond_pose[bone_id].matrix();

            /*let anim = &props.frames.as_ref().unwrap()[bone_id];
            let frame_index = anim
                .iter()
                .position(|t| t.timestamp > anim_timestamp)
                .unwrap_or(0);

            let mut frame_mat = anim[frame_index].rotation.to_rotate_matrix();
            frame_mat[0][3] = anim[frame_index].position.x;
            frame_mat[1][3] = anim[frame_index].position.y;
            frame_mat[2][3] = anim[frame_index].position.z;*/

            let frame_t = self.bones[bone_id].transform();

            let v = self.geometry.vertices.position(i).unwrap();
            let v = Vec3::crossed_mat(
                &Vec3::crossed_mat(v, &bond_pose_mat),
                &frame_t.as_ref().borrow().matrix(),
            );

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
        for bone_id in 0..self.bones.len() {
            let props = self.props();
            let anim = &props.frames.as_ref().unwrap()[bone_id];
            let frame_index = anim
                .iter()
                .position(|t| t.timestamp > self.props().last_time)
                .unwrap_or(0);
            let mut frame_mat = anim[frame_index].rotation.to_rotate_matrix();
            frame_mat[0][3] = anim[frame_index].position.x;
            frame_mat[1][3] = anim[frame_index].position.y;
            frame_mat[2][3] = anim[frame_index].position.z;

            let b = self.bones[bone_id].transform();
            let mut t = b.borrow_mut();
            t.set_matrix(frame_mat);
        }

        let rc = self.entity.get_rendering_component().unwrap();
        let objects = rc.render_objects();

        if objects.len() > 0 {
            let ro = &objects[0];
            ro.update_vertices(&|vb: RefMut<VertexBuffer>| {
                self.update_vertex_buffer(self.props().last_time, vb);
            });

            let last_time = self.props().last_time;
            self.props_mut().last_time = last_time + delta_sec;

            if self.props().last_time > self.props().max_time {
                self.props_mut().last_time = 0.;
            }
        }
    }
}
