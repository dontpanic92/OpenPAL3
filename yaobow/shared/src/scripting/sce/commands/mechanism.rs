use crate::{
    openpal3::{directors::SceneManagerExtensions, scene::ScnScene},
    scripting::sce::{SceCommand, SceState},
};
use crosscom::ComRc;
use imgui::Ui;
use radiance::comdef::ISceneManager;

#[derive(Debug, Clone)]
pub struct SceCommandTrigger {
    id: i32,
}

impl SceCommandTrigger {
    pub fn new(id: i32) -> Self {
        Self { id }
    }
}

impl SceCommand for SceCommandTrigger {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        if let Some(scene) = scene_manager.scn_scene() {
            let scene = scene.inner::<ScnScene>();
            scene.trigger_mechanism(self.id as u16);
            scene.snapshot_mechanisms(&mut state.global_state_mut().persistent_state_mut());
            for proc_id in scene.take_mechanism_procs() {
                state.call_proc(proc_id);
            }
        }
        true
    }
}

#[derive(Debug, Clone)]
pub struct SceCommandGetSwitch {
    sub_scene: String,
    id: i32,
    var: i16,
}

impl SceCommandGetSwitch {
    pub fn new(sub_scene: String, id: i32, var: i16) -> Self {
        Self { sub_scene, id, var }
    }
}

impl SceCommand for SceCommandGetSwitch {
    fn update(
        &mut self,
        scene_manager: ComRc<ISceneManager>,
        _ui: &Ui,
        state: &mut SceState,
        _delta_sec: f32,
    ) -> bool {
        let switched = scene_manager
            .scn_scene()
            .map(|scene| {
                let scene = scene.inner::<ScnScene>();
                if scene.sub_name().eq_ignore_ascii_case(&self.sub_scene) {
                    scene.switch_state(self.id as u16)
                } else {
                    state
                        .global_state()
                        .persistent_state()
                        .mechanism_states(scene.name(), &self.sub_scene)
                        .and_then(|states| states.get(&(self.id as u16)))
                        .map(|s| s.switched)
                        .unwrap_or(0)
                }
            })
            .unwrap_or(0);
        state
            .context_mut()
            .current_proc_context_mut()
            .set_local(self.var, switched as i32);
        true
    }
}
