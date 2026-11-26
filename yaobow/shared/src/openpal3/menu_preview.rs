use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use crosscom::ComRc;
use fileformats::pal3::gdb::ItemDefinition;
use radiance::{
    comdef::{IEntity, IEntityExt, IScene},
    math::Vec3,
    rendering::{RenderTarget, RenderingEngine},
    scene::{CoreScene, ISceneExt},
};
use radiance_scripting::{
    comdef::services::IRenderTarget,
    services::{render_target::ScriptedRenderTarget, texture_cache::ImguiTextureCache},
};

use super::{asset_manager::AssetManager, loaders::pol::create_item_preview};

#[derive(Clone, Copy, PartialEq, Eq)]
struct Selection {
    id: i32,
    equipment: bool,
}

struct ActivePreview {
    scene: ComRc<IScene>,
    entity: ComRc<IEntity>,
    texture: ComRc<IRenderTarget>,
    target: Rc<RefCell<Box<dyn RenderTarget>>>,
}

impl Drop for ActivePreview {
    fn drop(&mut self) {
        self.scene.unload();
    }
}

pub(crate) struct MenuPreview {
    assets: Rc<AssetManager>,
    cache: Rc<RefCell<ImguiTextureCache>>,
    requested: Cell<Option<Selection>>,
    selected: Option<Selection>,
    active: Option<ActivePreview>,
    angle: f32,
    last_frame: Instant,
}

impl MenuPreview {
    pub fn new(assets: Rc<AssetManager>, cache: Rc<RefCell<ImguiTextureCache>>) -> Self {
        Self {
            assets,
            cache,
            requested: Cell::new(None),
            selected: None,
            active: None,
            angle: 0.,
            last_frame: Instant::now(),
        }
    }

    pub fn request(&self, id: i32, equipment: bool) -> i32 {
        let selection = Selection { id, equipment };
        self.requested.set(Some(selection));
        if self.selected == Some(selection) {
            self.active.as_ref().map_or(0, |p| p.texture.texture_id())
        } else {
            0
        }
    }

    pub fn render(&mut self, open: bool, engine: &mut dyn RenderingEngine) {
        // UI requests arrive while its texture cache is borrowed; prepare only
        // in the host's next pre-update, before entering another UI frame.
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        let selection = self.requested.take().filter(|_| open);
        if selection != self.selected {
            self.active = None;
            self.selected = selection;
            if let Some(selection) = selection {
                self.active = self.prepare(selection);
            }
        }
        let Some(preview) = &mut self.active else {
            return;
        };
        // PAL3unpacked.exe 0x4390e2 / 0x439b0e: 98 degrees per second.
        self.angle = (self.angle + dt * 98.).rem_euclid(360.);
        preview
            .entity
            .transform()
            .borrow_mut()
            .clear_rotation()
            .rotate_axis_angle(&Vec3::new(0., 1., 0.), self.angle.to_radians());
        preview.scene.update(0.);
        engine.render_scene_to_target(preview.scene.clone(), &mut **preview.target.borrow_mut());
    }

    fn prepare(&self, selection: Selection) -> Option<ActivePreview> {
        let data = self.assets.stats_data();
        let item = data.item(selection.id as u32)?;
        let (model, texture) = item_paths(item)?;
        let entity = match create_item_preview(
            &self.assets.component_factory(),
            self.assets.vfs(),
            &model,
            &texture,
        ) {
            Ok(entity) => entity,
            Err(error) => {
                log::warn!("Cannot preview PAL3 item {}: {error}", selection.id);
                return None;
            }
        };
        let scene = CoreScene::create();
        scene.add_entity(entity.clone());
        scene.load();
        {
            let mut camera = scene.camera_mut();
            let (size, x, y) = framing(item, selection.equipment);
            camera.set_aspect(800. / 600.);
            camera.set_orthographic(size * 1.5);
            // Native 0x439930: screen anchor -> orthographic look-at.
            let x = -(x * 2. / 800. - 1.) * size;
            let y = -(1. - y * 2. / 600.) * size;
            camera
                .transform_mut()
                .set_position(&Vec3::new(x, y + 600., 1600.))
                .look_at(&Vec3::new(x, y, 0.));
        }
        let mut target = self
            .assets
            .component_factory()
            .create_render_target(800, 600);
        target.set_clear_color([0., 0., 0., 0.]);
        let (texture, target) = ScriptedRenderTarget::create(target, self.cache.clone());
        Some(ActivePreview {
            scene,
            entity,
            texture,
            target,
        })
    }
}

fn item_paths(item: &ItemDefinition) -> Option<(String, String)> {
    if item.model.is_empty() || item.model.eq_ignore_ascii_case("NONE") {
        return None;
    }
    let folder = format!("/basedata/basedata/Item/{}", item.model);
    Some((
        format!("{folder}/{}.pol", item.model),
        format!("{folder}/{}{}.tga", item.model, item.image),
    ))
}

fn framing(item: &ItemDefinition, equipment: bool) -> (f32, f32, f32) {
    // Native inventory 0x48e487; equipment selection 0x4a0e70.
    if !equipment {
        (160., 110., 268.)
    } else if item.category == 11 {
        (228., 128., 262.)
    } else if item.category == 6 {
        (200., 128., 258.)
    } else if item.id == 6421 {
        (160., 128., 276.)
    } else {
        (160., 128., 258.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_preview_uses_model_plus_texture_suffix_and_native_framing() {
        let mut item = ItemDefinition {
            id: 80010,
            model: "MB".into(),
            image: "6".into(),
            category: 6,
            ..Default::default()
        };
        assert_eq!(
            item_paths(&item),
            Some((
                "/basedata/basedata/Item/MB/MB.pol".into(),
                "/basedata/basedata/Item/MB/MB6.tga".into()
            ))
        );
        assert_eq!(framing(&item, false), (160., 110., 268.));
        assert_eq!(framing(&item, true), (200., 128., 258.));
        item.category = 11;
        assert_eq!(framing(&item, true), (228., 128., 262.));
        item.category = 0;
        item.id = 6421;
        assert_eq!(framing(&item, true), (160., 128., 276.));
        item.id = 0;
        assert_eq!(framing(&item, true), (160., 128., 258.));
        item.model.clear();
        assert_eq!(item_paths(&item), None);
        item.model = "NONE".into();
        assert_eq!(item_paths(&item), None);
    }
}
