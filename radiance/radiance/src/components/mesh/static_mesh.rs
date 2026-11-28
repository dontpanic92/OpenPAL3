use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crosscom::ComRc;

use crate::{
    comdef::{IComponentImpl, IEntity},
    math::Vec3,
    rendering::{ComponentFactory, MaterialDef},
};

use super::Geometry;
use crate::comdef::IEntityExt;

pub struct StaticMeshComponent {
    entity: ComRc<IEntity>,
    geometries: RefCell<Vec<Geometry>>,
    model_bounds: Cell<Option<(Vec3, Vec3)>>,
    model_sphere_center: Cell<Option<[f32; 3]>>,
    camera_clip_distance: Cell<Option<f32>>,
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
            model_bounds: Cell::new(None),
            model_sphere_center: Cell::new(None),
            camera_clip_distance: Cell::new(None),
            component_factory,
        }
    }

    /// Borrow the component's geometry list. Returns a `Ref` so callers
    /// can iterate without copying — but cannot hold it across calls
    /// into [`replace_material`].
    pub fn get_geometries(&self) -> std::cell::Ref<'_, [Geometry]> {
        std::cell::Ref::map(self.geometries.borrow(), |v| v.as_slice())
    }

    pub fn model_bounds(&self) -> Option<(Vec3, Vec3)> {
        self.model_bounds.get()
    }

    pub fn set_model_bounds(&self, bounds: (Vec3, Vec3)) {
        self.model_bounds.set(Some(bounds));
    }

    pub fn model_sphere_center(&self) -> Option<[f32; 3]> {
        self.model_sphere_center.get()
    }

    pub fn set_model_sphere_center(&self, center: [f32; 3]) {
        self.model_sphere_center.set(Some(center));
    }

    pub fn enable_camera_clip(&self, distance: f32) -> bool {
        let Some(center) = self.model_sphere_center.get() else {
            return false;
        };
        if !self.component_factory.supports_camera_clip()
            || !center.iter().all(|v| v.is_finite())
            || !distance.is_finite()
            || distance < 0.
            || self.geometries.borrow().iter().any(|geometry| {
                geometry.material.program() != crate::rendering::ShaderProgram::TexturedNoLight
            })
        {
            return false;
        }
        self.camera_clip_distance.set(Some(distance));
        if let Some(rendering) = self.entity.get_rendering_component() {
            for object in rendering.render_objects() {
                if !object.as_dyn().set_camera_clip(Some((center, distance))) {
                    self.disable_camera_clip();
                    return false;
                }
            }
        }
        true
    }

    pub fn disable_camera_clip(&self) {
        self.camera_clip_distance.set(None);
        if let Some(rendering) = self.entity.get_rendering_component() {
            for object in rendering.render_objects() {
                object.as_dyn().set_camera_clip(None);
            }
        }
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
                if let Some(distance) = self.camera_clip_distance.get() {
                    ro.as_dyn()
                        .set_camera_clip(Some((self.model_sphere_center.get().unwrap(), distance)));
                }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::{
        RenderObject, RenderObjectHandle, RenderTarget, RenderingComponent, ShaderProgram, Texture,
        TextureDef, VertexBuffer, VertexComponents, VideoPlayer,
    };

    struct ClipFactory {
        calls: Rc<RefCell<Vec<Option<([f32; 3], f32)>>>>,
    }

    struct ClipObject(Rc<RefCell<Vec<Option<([f32; 3], f32)>>>>);

    impl RenderObject for ClipObject {
        fn update_vertices(&self, _: &dyn Fn(std::cell::RefMut<VertexBuffer>)) {}
        fn set_camera_clip(&self, clip: Option<([f32; 3], f32)>) -> bool {
            self.0.borrow_mut().push(clip);
            true
        }
    }

    impl ComponentFactory for ClipFactory {
        fn supports_camera_clip(&self) -> bool {
            true
        }
        fn create_render_object(
            &self,
            _: VertexBuffer,
            _: Vec<u32>,
            _: &MaterialDef,
            _: bool,
        ) -> RenderObjectHandle {
            RenderObjectHandle::from_dyn(Rc::new(ClipObject(self.calls.clone())))
        }
        fn create_rendering_component(
            &self,
            objects: Vec<RenderObjectHandle>,
        ) -> RenderingComponent {
            let mut component = RenderingComponent::new();
            for object in objects {
                component.push_render_object(object);
            }
            component
        }
        fn create_texture(&self, _: &TextureDef) -> Box<dyn Texture> {
            unreachable!()
        }
        fn create_imgui_texture(
            &self,
            _: &[u8],
            _: u32,
            _: u32,
            _: u32,
            _: Option<imgui::TextureId>,
        ) -> (Box<dyn Texture>, imgui::TextureId) {
            unreachable!()
        }
        fn remove_imgui_texture(&self, _: Option<imgui::TextureId>) {
            unreachable!()
        }
        fn create_video_player(&self) -> Box<VideoPlayer> {
            unreachable!()
        }
        fn create_render_target(&self, _: u32, _: u32) -> Box<dyn RenderTarget> {
            unreachable!()
        }
    }

    #[test]
    fn camera_clip_registration_survives_deferred_mesh_loading_and_can_be_disabled() {
        let entity = crate::scene::CoreEntity::create("deferred clip".into(), true);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let geometry = Geometry {
            vertices: VertexBuffer::new(VertexComponents::POSITION, 1),
            indices: vec![0],
            material: MaterialDef::builder(ShaderProgram::TexturedNoLight).build(),
        };
        let mesh = StaticMeshComponent::new(
            entity.clone(),
            vec![geometry],
            Rc::new(ClipFactory {
                calls: calls.clone(),
            }),
        );
        assert!(!mesh.enable_camera_clip(1200.));
        mesh.set_model_sphere_center([10., 20., 30.]);
        assert!(mesh.enable_camera_clip(1200.));
        assert!(calls.borrow().is_empty());
        assert!(entity.get_rendering_component().is_none());
        mesh.on_loading();
        assert_eq!(*calls.borrow(), [Some(([10., 20., 30.], 1200.))]);
        mesh.disable_camera_clip();
        assert_eq!(calls.borrow().last(), Some(&None));
        assert!(mesh.enable_camera_clip(1200.));
        assert_eq!(calls.borrow().last(), Some(&Some(([10., 20., 30.], 1200.))));
        let count = calls.borrow().len();
        assert!(!mesh.enable_camera_clip(f32::NAN));
        assert_eq!(calls.borrow().len(), count);
    }
}
