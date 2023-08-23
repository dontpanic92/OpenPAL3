use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use crosscom::ComRc;

use crate::{
    comdef::{IAnimatedMeshComponentImpl, IComponentImpl, IEntity},
    math::{Vec2, Vec3},
    rendering::{ComponentFactory, VertexBuffer, VertexComponents},
    ComObject_AnimatedMeshComponent,
};

use super::{morph_target::MorphTarget, Geometry, TexCoord};

pub struct AnimatedMeshComponent {
    entity: ComRc<IEntity>,
    component_factory: Rc<dyn ComponentFactory>,
    props: RefCell<AnimatedMeshComponentProps>,
}

#[derive(PartialEq, Copy, Clone)]
pub enum MorphAnimationState {
    NotStarted,
    Playing,
    Holding,
    Finished,
}

struct AnimatedMeshComponentProps {
    morph_targets: Vec<MorphTarget>,
    morph_animation_state: MorphAnimationState,
    last_time: f32,
    hold_time: f32,
    hold: bool,
}

ComObject_AnimatedMeshComponent!(super::AnimatedMeshComponent);

impl AnimatedMeshComponent {
    pub fn new(entity: ComRc<IEntity>, component_factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            entity,
            component_factory,
            props: RefCell::new(AnimatedMeshComponentProps {
                morph_targets: vec![],
                morph_animation_state: MorphAnimationState::NotStarted,
                last_time: 0.,
                hold_time: 0.,
                hold: false,
            }),
        }
    }

    fn props_mut(&self) -> RefMut<AnimatedMeshComponentProps> {
        self.props.borrow_mut()
    }

    fn props(&self) -> Ref<AnimatedMeshComponentProps> {
        self.props.borrow()
    }

    pub fn morph_animation_length(&self) -> Option<f32> {
        self.props()
            .morph_targets
            .last()
            .and_then(|m| Some(m.timestamp))
    }

    pub fn add_morph_last_time(&self, time: f32) {
        self.props_mut().last_time += time;
    }

    pub fn reset_morph_last_time(&self) {
        self.props_mut().last_time = 0.;
    }

    pub fn set_morph_hold(&self) {
        self.props_mut().hold = true;
    }

    pub fn reset_morph_hold(&self) {
        self.props_mut().hold = false;
    }

    pub fn is_to_hold(&self) -> bool {
        !self.props().hold
            && self.props().hold_time > 0.
            && self.props().last_time > self.props().hold_time
    }

    pub fn is_to_end(&self) -> bool {
        self.props().last_time > self.morph_animation_length().unwrap()
    }

    pub fn set_morph_targets(&self, morph_targets: Vec<MorphTarget>, hold_time: f32) {
        self.props_mut().morph_targets = morph_targets;
        self.props_mut().morph_animation_state = MorphAnimationState::Playing;
        self.props_mut().hold_time = hold_time;
        self.reset_morph_last_time();

        if self.props().morph_targets.is_empty() {
            return;
        }

        self.load_geometries(&self.props().morph_targets[0].geometries);
    }

    pub fn update_morph_target(
        &self,
        anim_timestamp: f32,
        g_index: usize,
        mut vertex_buffer: RefMut<VertexBuffer>,
    ) {
        let props = self.props();
        let frame_index = props
            .morph_targets
            .iter()
            .position(|t| t.timestamp > anim_timestamp)
            .unwrap_or(0)
            - 1;

        let next_frame_index = (frame_index + 1) % props.morph_targets.len();
        let percentile = (anim_timestamp - props.morph_targets[frame_index].timestamp)
            / (props.morph_targets[next_frame_index].timestamp
                - props.morph_targets[frame_index].timestamp);

        let target = &props.morph_targets.get(frame_index).unwrap().geometries;
        let next_target = &props
            .morph_targets
            .get(next_frame_index)
            .unwrap()
            .geometries;

        // for (t, nt) in target.iter().zip(next_target) {
        let t = target.get(g_index);
        let nt = next_target.get(g_index);
        if t.is_none() || nt.is_none() {
            return;
        }

        let t = t.unwrap();
        let nt = nt.unwrap();

        for i in 0..vertex_buffer.count() {
            let position = t.vertices.position(i).unwrap();
            let next_position = nt.vertices.position(i).unwrap();
            let tex_coord = t.vertices.tex_coord(i);

            vertex_buffer.set_component(i, VertexComponents::POSITION, |p: &mut Vec3| {
                p.x = position.x * (1. - percentile) + next_position.x * percentile;
                p.y = position.y * (1. - percentile) + next_position.y * percentile;
                p.z = position.z * (1. - percentile) + next_position.z * percentile;
            });

            if let Some(tex_coord) = tex_coord {
                vertex_buffer.set_component(i, VertexComponents::TEXCOORD, |t: &mut Vec2| {
                    t.x = tex_coord.x;
                    t.y = tex_coord.y;
                });
            }
        }
        // }
    }

    pub fn blend_morph_target(&self, anim_timestamp: f32) -> Vec<Geometry> {
        let props = self.props();
        let frame_index = props
            .morph_targets
            .iter()
            .position(|t| t.timestamp > anim_timestamp)
            .unwrap_or(0)
            - 1;

        let next_frame_index = (frame_index + 1) % props.morph_targets.len();
        let percentile = (anim_timestamp - props.morph_targets[frame_index].timestamp)
            / (props.morph_targets[next_frame_index].timestamp
                - props.morph_targets[frame_index].timestamp);

        let target = &props.morph_targets.get(frame_index).unwrap().geometries;
        let next_target = &props
            .morph_targets
            .get(next_frame_index)
            .unwrap()
            .geometries;

        let mut blended = vec![];
        for (t, nt) in target.iter().zip(next_target) {
            let mut vertices = vec![];
            let mut normals = vec![];
            let mut texcoord_vec = vec![];
            let mut texcoord2_vec = vec![];

            for i in 0..t.vertices.count() {
                let position = t.vertices.position(i).unwrap();
                let next_position = nt.vertices.position(i).unwrap();
                vertices.push(Self::blend_vec3(position, next_position, percentile));

                let normal = t.vertices.normal(i);
                if let Some(normal) = normal {
                    let next_normal = nt.vertices.normal(i).unwrap();
                    normals.push(Self::blend_vec3(normal, next_normal, percentile));
                }

                let tex_coord = t.vertices.tex_coord(i);
                if let Some(tex_coord) = tex_coord {
                    texcoord_vec.push(TexCoord::new(tex_coord.x, tex_coord.y));
                }

                let tex_coord2 = t.vertices.tex_coord2(i);
                if let Some(tex_coord) = tex_coord2 {
                    texcoord2_vec.push(TexCoord::new(tex_coord.x, tex_coord.y));
                }
            }

            let mut texcoords = vec![];
            if texcoord_vec.len() > 0 {
                texcoords.push(texcoord_vec);
            }

            if texcoord2_vec.len() > 0 {
                texcoords.push(texcoord2_vec);
            }

            let normals = if normals.len() > 0 {
                Some(normals.as_ref())
            } else {
                None
            };

            let geometry = Geometry::new(
                &vertices,
                normals,
                &texcoords,
                t.indices.clone(),
                t.material.clone(),
                1,
            );
            blended.push(geometry);
        }

        blended
    }

    fn blend_vec3(v1: &Vec3, v2: &Vec3, v2_p: f32) -> Vec3 {
        Vec3::new(
            v1.x * (1. - v2_p) + v2.x * v2_p,
            v1.y * (1. - v2_p) + v2.y * v2_p,
            v1.z * (1. - v2_p) + v2.z * v2_p,
        )
    }

    fn _blend_vec2(v1: &Vec2, v2: &Vec2, v2_p: f32) -> Vec2 {
        Vec2::new(
            v1.x * (1. - v2_p) + v2.x * v2_p,
            v1.y * (1. - v2_p) + v2.y * v2_p,
        )
    }

    fn load_geometries(&self, geometries: &[Geometry]) {
        let mut objects = vec![];
        for geometry in geometries {
            let ro = self.component_factory.create_render_object(
                geometry.vertices.clone(),
                geometry.indices.clone(),
                &geometry.material,
                true,
            );

            objects.push(ro);
        }

        let component = self.component_factory.create_rendering_component(objects);
        self.entity
            .set_rendering_component(Some(Rc::new(component)));
    }

    fn render(&self, timestamp: f32) {
        if self.entity.get_rendering_component().is_none() {
            return;
        }

        let rc = self.entity.get_rendering_component().unwrap();
        let objects = rc.render_objects();

        for i in 0..objects.len() {
            let ro = &objects[i];
            ro.update_vertices(&|vb: RefMut<VertexBuffer>| {
                self.update_morph_target(timestamp, i, vb);
            });
        }
    }
}

impl IComponentImpl for AnimatedMeshComponent {
    fn on_loading(&self) -> crosscom::Void {
        self.load_geometries(&self.props().morph_targets[0].geometries);
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {
        let last_time = self.props().last_time;
        let anim_state = self.props().morph_animation_state;

        if self.props().morph_targets.is_empty() {
            return;
        }

        if anim_state == MorphAnimationState::Playing {
            if self.is_to_end() {
                self.props_mut().morph_animation_state = MorphAnimationState::Finished;
                self.reset_morph_last_time();
                self.reset_morph_hold();
                return;
            } else if self.is_to_hold() {
                self.props_mut().morph_animation_state = MorphAnimationState::Holding;
                self.set_morph_hold();
            } else {
                self.add_morph_last_time(delta_sec);
            }

            self.render(last_time);
        } else if anim_state == MorphAnimationState::Holding {
            self.render(last_time);
        }
    }
}

impl IAnimatedMeshComponentImpl for AnimatedMeshComponent {
    fn morph_animation_state(&self) -> crate::components::mesh::MorphAnimationState {
        self.props().morph_animation_state
    }

    fn play(&self, replay: bool) -> () {
        if replay {
            self.reset_morph_last_time();
            self.reset_morph_hold();
        }
        self.props_mut().morph_animation_state = MorphAnimationState::Playing;
    }
}
