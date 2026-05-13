use crate::GameType;

use super::{main_content::ContentTabs, DevToolsAssetLoader, DevToolsState};
use crosscom::ComRc;
use imgui::Ui;
use p7::interpreter::context::Data;
use radiance::{
    audio::AudioEngine,
    comdef::{IDirector, IDirectorImpl, ISceneManager},
    math::Vec3,
    perf,
    radiance::UiManager,
    scene::CoreScene,
};
use radiance_editor::ui::window_content_rect;
use radiance_scripting::comdef::services::IVfsService;
use radiance_scripting::services::VfsService;
use radiance_scripting::ui_walker::{
    owned, walk, LocalCommandQueue, OwnedNode, TextureResolver, UiAdapter, WalkContext,
};
use radiance_scripting::{ScriptDirectorHandle, ScriptHost};
use std::{cell::RefCell, rc::Rc};

pub struct DevToolsDirector {
    scene_manager: ComRc<ISceneManager>,
    asset_mgr: DevToolsAssetLoader,
    ui: Rc<UiManager>,
    content_tabs: RefCell<ContentTabs>,
    tree_runtime: Rc<ScriptHost>,
    tree_root: ScriptDirectorHandle,
    tree_vfs: ComRc<IVfsService>,
}

ComObject_DevToolsDirector!(super::DevToolsDirector);

impl DevToolsDirector {
    pub fn new(
        audio_engine: Rc<dyn AudioEngine>,
        scene_manager: ComRc<ISceneManager>,
        asset_mgr: DevToolsAssetLoader,
        ui: Rc<UiManager>,
        game_type: GameType,
        script_runtime: Rc<ScriptHost>,
    ) -> ComRc<IDirector> {
        let tree_vfs = VfsService::create(asset_mgr.vfs_rc());
        let tree_root = init_tree_root(&script_runtime, tree_vfs.clone());
        ComRc::from_object(Self {
            scene_manager,
            content_tabs: RefCell::new(ContentTabs::new(
                audio_engine,
                asset_mgr.clone(),
                game_type,
            )),
            ui,
            asset_mgr,
            tree_runtime: script_runtime,
            tree_root,
            tree_vfs,
        })
    }

    fn main_window(&self, ui: &Ui) -> Option<DevToolsState> {
        let content_rect = window_content_rect(ui);
        let h_layout = content_rect.width > 800.;

        let sizes = {
            if h_layout {
                [
                    [content_rect.width * 0.3, content_rect.height],
                    [std::f32::MIN_POSITIVE, content_rect.height],
                ]
            } else {
                [
                    [content_rect.width, content_rect.height * 0.6],
                    [content_rect.width, std::f32::MIN_POSITIVE],
                ]
            }
        };

        ui.child_window("Files")
            .size(sizes[0])
            .build(|| self.render_tree(ui));

        if h_layout {
            ui.same_line();
        }

        let mut state = None;
        ui.child_window("Content")
            .size(sizes[1])
            .build(|| state = self.render_content(ui));

        state
    }

    fn render_tree(&self, ui: &Ui) {
        let _frame_timer = perf::timer("editor.tree.frame");
        let state = match self.tree_runtime.deref_handle(self.tree_root) {
            Some(state) => state,
            None => return,
        };

        let owned = {
            let node = {
                let _script_timer = perf::timer("editor.tree.script_render");
                self.tree_runtime
                    .call_method_returning_data(state, "render", vec![Data::Float(0.0)])
            };
            match node {
                Ok(node) => {
                    let _resolve_timer = perf::timer("editor.tree.resolve");
                    match self.tree_runtime.with_ctx(|ctx| owned::resolve(ctx, &node)) {
                        Ok(owned) => Some(owned),
                        Err(err) => {
                            log::error!("scripted resource tree resolve failed: {}", err);
                            None
                        }
                    }
                }
                Err(err) => {
                    log::error!("scripted resource tree render failed: {}", err);
                    None
                }
            }
        };

        let Some(owned) = owned.as_ref() else {
            return;
        };
        perf::gauge("editor.tree.owned_nodes", count_owned_nodes(owned) as u64);

        let mut textures = NullTextureResolver;
        let mut queue = LocalCommandQueue::default();
        let fonts: Vec<imgui::FontId> = ui.fonts().fonts().to_vec();
        let mut adapter = UiAdapter {
            ui,
            ctx: WalkContext {
                textures: &mut textures,
                commands: &mut queue,
                fonts: &fonts,
                dpi_scale: self.ui.dpi_scale(),
            },
            table_counter: std::cell::Cell::new(0),
        };
        {
            let _walk_timer = perf::timer("editor.tree.walk");
            if let Err(err) = walk(owned, &mut adapter) {
                log::error!("scripted resource tree walk failed: {:?}", err);
            }
        }
        perf::gauge("editor.tree.commands", queue.queue.len() as u64);

        for command_id in queue.queue {
            let path = self.tree_vfs.command_path(command_id).to_string();
            if path.is_empty() {
                log::warn!(
                    "scripted resource tree command {} resolved to an empty path",
                    command_id
                );
                continue;
            }
            if self.tree_vfs.is_dir(&path) {
                log::info!("scripted resource tree toggling directory '{}'", path);
                self.tree_vfs.toggle_expanded(&path);
            } else {
                log::info!("scripted resource tree opening file '{}'", path);
                self.content_tabs
                    .borrow_mut()
                    .open(self.asset_mgr.vfs(), path);
            }
        }
    }

    fn render_content(&self, ui: &Ui) -> Option<DevToolsState> {
        self.content_tabs.borrow_mut().render_tabs(ui)
    }
}

impl Drop for DevToolsDirector {
    fn drop(&mut self) {
        self.tree_runtime.unroot(self.tree_root);
    }
}

struct NullTextureResolver;

impl TextureResolver for NullTextureResolver {
    fn resolve(&mut self, _com_id: i64) -> Option<imgui::TextureId> {
        None
    }
}

fn count_owned_nodes(node: &OwnedNode) -> usize {
    1 + node.children.iter().map(count_owned_nodes).sum::<usize>()
}

fn init_tree_root(host: &Rc<ScriptHost>, vfs: ComRc<IVfsService>) -> ScriptDirectorHandle {
    let vfs_id = host.intern(vfs);
    let vfs = host
        .foreign_box("radiance_scripting.comdef.services.IVfsService", vfs_id)
        .expect("scripted resource tree VFS service must be internable");
    let tree = host
        .call_returning_data("init_resource_tree", vec![vfs])
        .expect("resource tree script init must succeed");
    host.root(tree)
}

impl IDirectorImpl for DevToolsDirector {
    fn activate(&self) {}

    fn update(&self, _delta_sec: f32) -> Option<ComRc<IDirector>> {
        let ui = self.ui.ui();
        let state = self.main_window(ui);
        perf::flush_frame();
        match state {
            Some(DevToolsState::PreviewScene { cpk_name, scn_name }) => {
                self.scene_manager.pop_scene();

                let scene = self
                    .asset_mgr
                    .pal3()
                    .unwrap()
                    .load_scn(cpk_name.as_str(), scn_name.as_str());
                scene
                    .camera()
                    .borrow_mut()
                    .transform_mut()
                    .set_position(&Vec3::new(0., 500., 500.))
                    .look_at(&Vec3::new(0., 0., 0.));
                self.scene_manager.push_scene(scene);
            }
            Some(DevToolsState::PreviewEntity(entity)) => {
                let scene = CoreScene::create();
                self.scene_manager.pop_scene();
                self.scene_manager.push_scene(scene.clone());

                entity.load();
                scene.add_entity(entity);
                scene
                    .camera()
                    .borrow_mut()
                    .transform_mut()
                    .set_position(&Vec3::new(0., 200., 200.))
                    .look_at(&Vec3::new(0., 0., 0.));
            }
            Some(DevToolsState::MainWindow) => {}
            None => {}
        }

        None
    }
}
