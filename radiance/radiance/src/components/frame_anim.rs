//! Grid-atlas sprite-animation component.
//!
//! Plays a frame sequence packed into a single texture atlas (a
//! `cols × rows` grid) by driving its owning entity's render-object
//! material UV affine (`set_uv_xform`) every frame so the shader's
//! `uv = inTexCoord * scale + offset` addresses the current cell. The
//! quad/geometry is authored with full-cell UVs in `[0, 1]`; this
//! component scales them down to one cell and offsets to the active
//! frame.
//!
//! It **self-ticks** via `IComponent::on_updating` (the scene/entity
//! dispatches the per-frame update), so callers attach it once and are
//! done. Used by PAL3 `EffectScn` effects (candle / torch / fire flames)
//! to drive an additive billboard through its frame loop.
//!
//! The animated material must be `make_unique()` so the renderer
//! allocates a per-object params UBO for it (otherwise the UV affine
//! would leak onto every material sharing the same cache key).

use std::cell::Cell;

use crosscom::ComRc;

use crate::comdef::{IComponentImpl, IEntity, IEntityExt};

pub struct FrameAnimationComponent {
    entity: ComRc<IEntity>,
    cols: u32,
    rows: u32,
    frames: u32,
    fps: f32,
    elapsed: Cell<f32>,
}

ComObject_FrameAnimationComponent!(super::FrameAnimationComponent);

impl FrameAnimationComponent {
    pub fn create(
        entity: ComRc<IEntity>,
        cols: u32,
        rows: u32,
        frames: u32,
        fps: f32,
    ) -> ComRc<crate::comdef::IFrameAnimationComponent> {
        ComRc::from_object(Self {
            entity,
            cols: cols.max(1),
            rows: rows.max(1),
            frames: frames.max(1),
            fps: fps.max(0.001),
            elapsed: Cell::new(0.0),
        })
    }

    fn apply_frame(&self, col: u32, row: u32) {
        let Some(rc) = self.entity.get_rendering_component() else {
            return;
        };
        let scale = [1.0 / self.cols as f32, 1.0 / self.rows as f32];
        let offset = [col as f32 / self.cols as f32, row as f32 / self.rows as f32];
        for obj in rc.render_objects() {
            obj.as_dyn().set_uv_xform(scale, offset);
        }
    }
}

impl IComponentImpl for FrameAnimationComponent {
    fn on_loading(&self) -> crosscom::Void {}

    fn on_updating(&self, delta_sec: f32) -> crosscom::Void {
        let t = self.elapsed.get() + delta_sec;
        self.elapsed.set(t);
        let frame = ((t * self.fps) as u32) % self.frames;
        let col = frame % self.cols;
        let row = frame / self.cols;
        self.apply_frame(col, row);
    }

    fn on_unloading(&self) {}
}
