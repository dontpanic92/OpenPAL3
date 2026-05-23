use std::cell::RefCell;
use std::rc::Rc;

use crosscom::ComRc;

use crate::{
    comdef::{IComponentImpl, IEntity},
    rendering::{ComponentFactory, MaterialDef},
};

use super::Geometry;
use crate::comdef::IEntityExt;

pub struct StaticMeshComponent {
    entity: ComRc<IEntity>,
    geometries: RefCell<Vec<Geometry>>,
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
            geometries: RefCell::new(geometries),
            component_factory,
        }
    }

    /// Borrow the component's geometry list. Returns a `Ref` so callers
    /// can iterate without copying — but cannot hold it across calls
    /// into [`replace_material`].
    pub fn get_geometries(&self) -> std::cell::Ref<'_, [Geometry]> {
        std::cell::Ref::map(self.geometries.borrow(), |v| v.as_slice())
    }

    /// Replace the material on geometry `idx`. Must be called before
    /// the owning entity is added to a scene — once
    /// `on_loading` has run, the per-geometry render objects have
    /// already been built from the original `MaterialDef` and a later
    /// mutation here is silently dropped on the floor.
    ///
    /// Used by the PAL4 nav-mesh debug visualization to swap the
    /// floor/wall DFFs' diffuse materials for a `GradientYMaterialDef`
    /// keyed on the scene's world-Y range.
    pub fn replace_material(&self, idx: usize, material: MaterialDef) {
        let mut geometries = self.geometries.borrow_mut();
        if let Some(g) = geometries.get_mut(idx) {
            g.material = material;
        }
    }

    /// Number of geometries in this component. Convenience for
    /// callers that want to iterate `0..len` and call
    /// `replace_material` without holding a borrow.
    pub fn geometry_count(&self) -> usize {
        self.geometries.borrow().len()
    }
}

impl IComponentImpl for StaticMeshComponent {
    fn on_loading(&self) -> crosscom::Void {
        let mut objects = vec![];
        for geometry in self.geometries.borrow().iter() {
            if geometry.indices.len() != 0 {
                let ro = self.component_factory.create_render_object(
                    geometry.vertices.clone(),
                    geometry.indices.clone(),
                    &geometry.material,
                    false,
                );

                objects.push(ro);
            }
        }

        let component = self.component_factory.create_rendering_component(objects);
        self.entity
            .set_rendering_component(Some(Rc::new(component)));
    }

    fn on_unloading(&self) {}

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {}
}
