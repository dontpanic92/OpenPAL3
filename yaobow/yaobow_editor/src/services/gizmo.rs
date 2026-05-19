//! Display-only selection gizmo for the scene preview.
//!
//! v1 keeps it minimal: one "pivot indicator" cube anchored at the
//! world position of the selected entity. The cube material is
//! `SimpleMaterialDef::create2(..., None)` (default white) so we don't
//! need to bake a coloured texture.
//!
//! A future iteration can extend `Gizmo` with three coloured axis
//! triad entities and an AABB outline made of 12 thin elongated
//! boxes — see `plan.md`.

use std::rc::Rc;

use crosscom::ComRc;
use radiance::comdef::{IEntity, IEntityExt, IScene};
use radiance::comdef::ISceneExt;
use radiance::debug;
use radiance::math::Vec3;
use radiance::rendering::ComponentFactory;

/// Selection-gizmo entities owned by a `ScenePreviewSession`. The
/// entities live inside the preview scene so they share the orbit
/// camera and offscreen pipeline.
pub struct Gizmo {
    pivot: ComRc<IEntity>,
    axes: [ComRc<IEntity>; 3],
}

impl Gizmo {
    /// Spawn the gizmo entities into `scene`. They start hidden and
    /// only become visible after `update(...)` is called with a real
    /// selection. The pivot cube is rendered with the default
    /// `SimpleMaterialDef` (white, lit) which is sufficient for a
    /// read-only "you selected this" marker.
    pub fn create(scene: &ComRc<IScene>, factory: Rc<dyn ComponentFactory>) -> Self {
        let pivot = debug::create_box_entity(factory.clone());
        pivot.set_visible(false);
        pivot.set_enabled(false);
        scene.add_entity(pivot.clone());

        // The three axis triad spikes are made from elongated unit
        // boxes; we'd ideally tint them red/green/blue, but
        // SimpleMaterialDef doesn't expose vertex colors and we don't
        // want to bake three 1×1 textures. The triad still gives a
        // useful "where is the origin / which way is up" indication.
        let axis = |length: f32, axis_dir: Vec3| {
            let l = length * 0.5;
            // Slim box aligned to `axis_dir`. The mesh in
            // `create_box_entity2` takes 8 corners and triangulates
            // them as a box, so we hand it a thin slab along the
            // requested axis.
            let r = 1.0;
            let along = axis_dir;
            let p = |a: f32, dx: f32, dy: f32, dz: f32| {
                Vec3::new(
                    along.x * a + dx,
                    along.y * a + dy,
                    along.z * a + dz,
                )
            };
            let verts = vec![
                p(-l, -r, -r, 0.),
                p(-l, -r, r, 0.),
                p(-l, r, r, 0.),
                p(-l, r, -r, 0.),
                p(l, -r, -r, 0.),
                p(l, -r, r, 0.),
                p(l, r, r, 0.),
                p(l, r, -r, 0.),
            ];
            let e = debug::create_box_entity2(factory.clone(), verts);
            e.set_visible(false);
            e.set_enabled(false);
            scene.add_entity(e.clone());
            e
        };
        let axes = [
            axis(50.0, Vec3::new(1., 0., 0.)),
            axis(50.0, Vec3::new(0., 1., 0.)),
            axis(50.0, Vec3::new(0., 0., 1.)),
        ];

        Self { pivot, axes }
    }

    /// Re-position the gizmo at `world_pos` and apply the visibility
    /// toggles. Passing `world_pos = None` hides everything regardless
    /// of the per-element flags.
    pub fn update(
        &self,
        world_pos: Option<Vec3>,
        show_axes: bool,
        show_aabb: bool,
        show_pivot: bool,
    ) {
        // `show_aabb` is reserved for a future implementation; the
        // pivot doubles as a selection box for v1. Keep the parameter
        // in the public surface so the IDL contract stays stable.
        let _ = show_aabb;

        let visible = world_pos.is_some();
        let pos = world_pos.unwrap_or_else(|| Vec3::new(0., 0., 0.));

        let set = |e: &ComRc<IEntity>, on: bool| {
            e.set_visible(on);
            e.set_enabled(on);
            e.transform().borrow_mut().set_position(&pos);
        };
        set(&self.pivot, visible && show_pivot);
        for a in &self.axes {
            set(a, visible && show_axes);
        }
    }
}
