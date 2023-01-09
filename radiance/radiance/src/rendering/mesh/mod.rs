pub mod geometry;

use std::{any::TypeId, rc::Rc};

use crate::{interfaces::IComponentImpl, scene::Entity, ComObject_MeshComponent};

use self::geometry::Geometry;

use super::{ComponentFactory, RenderingComponent};

pub struct MeshComponent {
    geometries: Vec<Geometry>,
    component_factory: Rc<dyn ComponentFactory>,
}

ComObject_MeshComponent!(super::MeshComponent);

impl MeshComponent {
    pub fn new(geometries: Vec<Geometry>, component_factory: Rc<dyn ComponentFactory>) -> Self {
        Self {
            geometries,
            component_factory,
        }
    }
}

impl IComponentImpl for MeshComponent {
    fn on_loading(&self, entity: &mut dyn Entity) -> crosscom::Void {
        let mut objects = vec![];
        for geometry in &self.geometries {
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

    fn on_updating(&self, entity: &mut dyn Entity, delta_sec: f32) -> crosscom::Void {}
}
