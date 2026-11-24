use radiance::math::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::openpal3::scene::MechanismState;
use crate::ydirs;

pub const PAL3_APP_NAME: &str = "OpenPAL3";

#[derive(Serialize, Deserialize)]
pub struct PersistentState {
    app_name: String,
    global_vars: HashMap<i16, i32>,
    position: Vec3,
    #[serde(default)]
    nav_layer: usize,
    #[serde(default)]
    role_controlled: i32,
    scene: Option<String>,
    sub_scene: Option<String>,
    #[serde(default)]
    scene_mechanisms: HashMap<String, HashMap<u16, MechanismState>>,
}

impl PersistentState {
    pub fn new(app_name: String) -> Self {
        Self {
            app_name,
            global_vars: HashMap::new(),
            position: Vec3::new(0., 0., 0.),
            nav_layer: 0,
            role_controlled: 0,
            scene: None,
            sub_scene: None,
            scene_mechanisms: HashMap::new(),
        }
    }

    fn get_data_dir(app_name: &str) -> PathBuf {
        ydirs::save_dir().join(app_name)
    }

    pub fn load(app_name: &str, slot: i32) -> Self {
        let path = Self::get_data_dir(app_name)
            .join("Save")
            .join(format!("{}.json", slot));
        let content = std::fs::read_to_string(path).unwrap();
        serde_json::from_str(&content).unwrap()
    }

    pub fn save(&self, slot: i32) {
        if slot >= 0 {
            let path = Self::get_data_dir(&self.app_name).join("Save");
            if let Err(e) = std::fs::create_dir_all(&path) {
                log::error!("Cannot create save dir: {}", e);
                return;
            }

            let result = serde_json::to_string_pretty(self);
            match result {
                Ok(content) => {
                    if let Err(e) = std::fs::write(path.join(format!("{}.json", slot)), content) {
                        log::error!("Cannot save: {}", e);
                    } else {
                        log::info!("Game saved");
                    }
                }
                Err(e) => log::error!("Cannot serialize persistent state: {}", e),
            };
        }
    }

    pub fn app_name(&self) -> &str {
        self.app_name.as_str()
    }

    pub fn set_global(&mut self, var: i16, value: i32) {
        self.global_vars.insert(var, value);
    }

    pub fn get_global(&self, var: i16) -> Option<i32> {
        self.global_vars.get(&var).and_then(|v| Some(*v))
    }

    pub fn position(&mut self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    pub fn nav_layer(&self) -> usize {
        self.nav_layer
    }

    pub fn set_nav_layer(&mut self, layer: usize) {
        self.nav_layer = layer;
    }

    pub fn role_controlled(&self) -> i32 {
        self.role_controlled
    }

    pub fn set_role_controlled(&mut self, role: i32) {
        self.role_controlled = role;
    }

    pub fn set_scene_name(&mut self, scene: String, sub_scene: String) {
        self.scene = Some(scene);
        self.sub_scene = Some(sub_scene);
    }

    pub fn scene_name(&self) -> Option<String> {
        self.scene.clone()
    }

    pub fn sub_scene_name(&self) -> Option<String> {
        self.sub_scene.clone()
    }

    pub fn mechanism_states(
        &self,
        scene: &str,
        sub_scene: &str,
    ) -> Option<&HashMap<u16, MechanismState>> {
        self.scene_mechanisms
            .get(&format!("{}/{}", scene, sub_scene).to_lowercase())
    }

    pub fn set_mechanism_states(
        &mut self,
        scene: &str,
        sub_scene: &str,
        states: HashMap<u16, MechanismState>,
    ) {
        self.scene_mechanisms
            .insert(format!("{}/{}", scene, sub_scene).to_lowercase(), states);
    }
}

#[cfg(test)]
mod mechanism_tests {
    use super::*;

    #[test]
    fn old_saves_without_mechanisms_remain_readable() {
        let mut json = serde_json::to_value(PersistentState::new("OpenPAL3".into())).unwrap();
        json.as_object_mut().unwrap().remove("scene_mechanisms");
        json.as_object_mut().unwrap().remove("nav_layer");
        json.as_object_mut().unwrap().remove("role_controlled");
        let state: PersistentState = serde_json::from_value(json).unwrap();
        assert!(state.mechanism_states("m17", "7").is_none());
    }

    #[test]
    fn mechanism_state_roundtrips_with_case_insensitive_scene_keys() {
        let mut state = PersistentState::new("OpenPAL3".into());
        state.set_role_controlled(1);
        state.set_nav_layer(1);
        state.set_mechanism_states(
            "M17",
            "D1",
            HashMap::from([(
                8,
                MechanismState {
                    remaining: 254,
                    switched: 1,
                    height: 25.,
                    raised: false,
                    motion: Some(40.),
                    animation_time: None,
                    native: None,
                    layer: None,
                    platform: None,
                    interaction: None,
                },
            )]),
        );
        let mut json = serde_json::to_value(&state).unwrap();
        for field in ["native", "layer", "platform", "interaction"] {
            json["scene_mechanisms"]["m17/d1"]["8"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        let restored: PersistentState = serde_json::from_value(json).unwrap();
        assert_eq!(restored.role_controlled(), 1);
        assert_eq!(restored.nav_layer(), 1);
        let mechanism = &restored.mechanism_states("m17", "d1").unwrap()[&8];
        assert_eq!(mechanism.switched, 1);
        assert_eq!(mechanism.remaining, 254);
        assert_eq!(mechanism.height, 25.);
        assert_eq!(mechanism.motion, Some(40.));
        assert!(restored.mechanism_states("m18", "d1").is_none());
    }
}
