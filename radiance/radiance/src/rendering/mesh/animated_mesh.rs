use std::{
    any::TypeId,
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use crate::{
    interfaces::IComponentImpl,
    math::{Vec2, Vec3},
    rendering::{ComponentFactory, RenderingComponent, VertexBuffer, VertexComponents},
    scene::Entity,
    ComObject_AnimatedMeshComponent,
};

use super::morph_target::MorphTarget;

pub struct AnimatedMeshComponent {
    component_factory: Rc<dyn ComponentFactory>,
    props: RefCell<AnimatedMeshComponentProps>,
}

struct AnimatedMeshComponentProps {
    morph_targets: Vec<MorphTarget>,
    morph_animation_enabled: bool,
    last_time: f32,
}

ComObject_AnimatedMeshComponent!(super::AnimatedMeshComponent);

impl AnimatedMeshComponent {
    pub fn new(component_factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            component_factory,
            props: RefCell::new(AnimatedMeshComponentProps {
                morph_targets: vec![],
                morph_animation_enabled: false,
                last_time: 0.,
            }),
        }
    }

    pub fn props_mut(&self) -> RefMut<AnimatedMeshComponentProps> {
        self.props.borrow_mut()
    }

    pub fn props(&self) -> Ref<AnimatedMeshComponentProps> {
        self.props.borrow()
    }

    pub fn morph_animation_length(&self) -> Option<f32> {
        self.props()
            .morph_targets
            .last()
            .and_then(|m| Some(m.timestamp))
    }

    pub fn reset_morph_last_time(&self) {
        self.props_mut().last_time = 0.;
    }

    pub fn set_morph_targets(&self, morph_targets: Vec<MorphTarget>) {
        self.props_mut().morph_targets = morph_targets;
        if self.props().morph_targets.is_empty() {
            return;
        }
    }

    pub fn update_morph_target(&self, anim_timestamp: f32, vertex_buffer: &mut VertexBuffer) {
        let props = self.props();
        let frame_index = props
            .morph_targets
            .iter()
            .position(|&t| t.timestamp > anim_timestamp)
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

        for (t, nt) in target.iter().zip(next_target) {
            for i in 0..t.vertices.count() {
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
        }
    }
}

impl IComponentImpl for AnimatedMeshComponent {
    fn on_loading(&self, entity: &mut dyn Entity) -> crosscom::Void {
        // TODO: deal with morph target change
        let geometries = &self.props().morph_targets[0].geometries;

        let mut objects = vec![];
        for geometry in geometries {
            let ro = self.component_factory.create_render_object(
                geometry.vertices.clone(),
                geometry.indices.clone(),
                &geometry.material,
                false,
            );

            objects.push(ro);
        }

        let component = self.component_factory.create_rendering_component(objects);
        entity.add_component(TypeId::of::<RenderingComponent>(), Box::new(component));
    }

    fn on_updating(&self, entity: &mut dyn Entity, delta_sec: f32) -> crosscom::Void {
        if self.props().morph_animation_enabled && !self.props().morph_targets.is_empty() {
            let anim_timestamp = self.props().last_time + delta_sec;
            if anim_timestamp > self.morph_animation_length().unwrap() {
                self.props_mut().morph_animation_enabled = false;
                self.reset_morph_last_time();
                return;
            }

            let rc = unsafe {
                let component = entity
                    .get_component_mut(TypeId::of::<RenderingComponent>())
                    .unwrap();

                &mut *(component
                    .as_mut()
                    .downcast_mut::<RenderingComponent>()
                    .unwrap() as *mut RenderingComponent)
            };
            let ro = rc.render_objects_mut().first_mut().unwrap();

            ro.update_vertices(&mut |vb: &mut VertexBuffer| {
                self.update_morph_target(anim_timestamp, vb);
            });

            self.props_mut().last_time = anim_timestamp;
        }
    }
}
