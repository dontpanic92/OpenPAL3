use std::rc::Rc;

use crosscom::ComRc;

use crate::{
    comdef::{IComponentImpl, IEntity},
    rendering::{ComponentFactory, Geometry},
    ComObject_StaticMeshComponent,
};

pub struct StaticMeshComponent {
    entity: ComRc<IEntity>,
    geometries: Vec<Geometry>,
    component_factory: Rc<dyn ComponentFactory>,
}

ComObject_StaticMeshComponent!(super::StaticMeshComponent);

impl StaticMeshComponent {
    pub fn new(
        entity: ComRc<IEntity>,
        geometries: Vec<Geometry>,
        component_factory: Rc<dyn ComponentFactory>,
    ) -> Self {
        Self {
            entity,
            geometries,
            component_factory,
        }
    }
}

impl IComponentImpl for StaticMeshComponent {
    fn on_loading(&self) -> crosscom::Void {
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
        self.entity
            .set_rendering_component(Some(Rc::new(component)));
    }

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {}
}
