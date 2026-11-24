use super::ScnScene;
use crate::openpal3::{
    asset_manager::AssetManager,
    loaders::{
        nav_loader::NavFile,
        scn_loader::{ScnFile, ScnNode},
    },
    states::persistent_state::PersistentState,
};
use radiance::{
    comdef::IEntityExt,
    math::Vec3,
    rendering::{
        ComponentFactory, MaterialDef, RenderObjectHandle, RenderTarget, RenderingComponent,
        Texture, TextureDef, VertexBuffer, VideoPlayer,
    },
    scene::{CoreEntity, CoreScene},
};
use std::rc::Rc;

struct UnusedFactory;

impl ComponentFactory for UnusedFactory {
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
    fn create_render_object(
        &self,
        _: VertexBuffer,
        _: Vec<u32>,
        _: &MaterialDef,
        _: bool,
    ) -> RenderObjectHandle {
        unreachable!()
    }
    fn create_rendering_component(&self, _: Vec<RenderObjectHandle>) -> RenderingComponent {
        unreachable!()
    }
    fn create_video_player(&self) -> Box<VideoPlayer> {
        unreachable!()
    }
    fn create_render_target(&self, _: u32, _: u32) -> Box<dyn RenderTarget> {
        unreachable!()
    }
}

fn node(id: u16, kind: u16, target: u16) -> ScnNode {
    ScnNode {
        index: id,
        active: 1,
        remaining_triggers: 255,
        switch_state: 0,
        name: String::new(),
        w24: 0,
        w26: 0,
        position: Vec3::new(0., 10., 0.),
        rotation: 0.,
        nav_trigger_coord_min: (0, 0),
        nav_trigger_coord_max: (0, 0),
        node_type: kind,
        nav_layer: 0,
        ladder_nav_coord1: (40, 0),
        ladder_nav_coord2: (0, 0),
        ladder_switch_layer: 0,
        b49: vec![],
        w66: vec![],
        b6e: vec![],
        sce_proc_id: 0,
        d84: 0,
        b88: vec![],
        w148: target,
        b14a: vec![0; 34],
        aabb_trigger_coord1: Vec3::new(0., 0., 0.),
        aabb_trigger_coord2: Vec3::new(0., 0., 0.),
        dw184: vec![],
        b: vec![],
    }
}

fn scene(nodes: Vec<ScnNode>) -> ScnScene {
    let scene = CoreScene::create();
    for node in &nodes {
        let entity = CoreEntity::create(format!("OBJECT_{}", node.index), true);
        entity.transform().borrow_mut().set_position(&node.position);
        scene.add_entity(entity);
    }
    let assets = Rc::new(AssetManager::new(
        Rc::new(UnusedFactory),
        Rc::new(mini_fs::MiniFs::new(true)),
    ));
    ScnScene::new(
        scene,
        &assets,
        "m17",
        "7",
        ScnFile {
            cpk_name: "m17".into(),
            scn_name: "7".into(),
            scn_base_name: "7".into(),
            global_scene_index: 0,
            is_night: false,
            skybox_id: 0,
            roles: vec![],
            nodes,
        },
        NavFile {
            version: 1,
            maps: vec![],
            unknown2: vec![],
        },
    )
}

#[test]
fn mechanism_links_wait_for_completion_and_survive_scene_restore() {
    let switch = node(9, 23, 3);
    let mut platform = node(3, 22, u16::MAX);
    platform.sce_proc_id = 507;
    let scene = scene(vec![switch.clone(), platform.clone()]);
    scene.trigger_mechanism(9);
    assert_eq!(scene.switch_state(9), 1);
    assert!(scene.update_mechanisms(1.).is_empty());
    let height = || {
        scene
            .get_root_object(3)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y
    };
    assert_eq!(height(), 10.);
    assert!(scene.update_mechanisms(0.5).is_empty());
    assert_eq!(height(), 25.);
    let mut saved = PersistentState::new("OpenPAL3".into());
    scene.snapshot_mechanisms(&mut saved);
    let restored = self::scene(vec![switch, platform]);
    restored.restore_mechanisms(&saved);
    assert_eq!(restored.switch_state(9), 1);
    assert_eq!(restored.update_mechanisms(0.5), vec![507]);
    assert_eq!(
        restored
            .get_root_object(3)
            .unwrap()
            .transform()
            .borrow()
            .position()
            .y,
        40.
    );
    assert!(restored.update_mechanisms(1.).is_empty());
}

#[test]
fn mechanism_self_links_are_deferred_instead_of_recursing() {
    let scene = scene(vec![node(9, 23, 9)]);
    scene.trigger_mechanism(9);
    assert_eq!(scene.switch_state(9), 1);
    scene.update_mechanisms(1.);
    assert_eq!(scene.switch_state(9), 0);
    scene.update_mechanisms(1.);
    assert_eq!(scene.switch_state(9), 1);
}

#[test]
fn mechanism_interaction_respects_layer_distance_and_runtime_visibility() {
    let mut switch = node(9, 23, u16::MAX);
    switch.ladder_nav_coord1 = (1, 2);
    switch.sce_proc_id = 500;
    let scene = scene(vec![switch]);
    let near = Vec3::new(0., 10., 0.);
    assert_eq!(scene.interact_mechanism(1, &near), None);
    assert_eq!(scene.interact_mechanism(0, &Vec3::new(201., 10., 0.)), None);
    assert_eq!(scene.interact_mechanism(0, &near), Some(Some(500)));
    assert_eq!(scene.switch_state(9), 0);
    scene.get_root_object(9).unwrap().set_visible(false);
    assert_eq!(scene.interact_mechanism(0, &near), None);
    scene.trigger_mechanism(9);
    assert_eq!(scene.switch_state(9), 1);
}
