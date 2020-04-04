use crate::loaders::cvdloader::*;
use crate::loaders::mv3loader::*;
use crate::loaders::polloader::*;
use crate::scene::ScnScene;
use std::path::{Path, PathBuf};

pub struct ResourceManager {
    root_path: PathBuf,
    scene_path: PathBuf,
    music_path: PathBuf,
    snd_path: PathBuf,
    basedata_path: PathBuf,
}

impl ResourceManager {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let root_path = path.as_ref().to_owned();
        let basedata_path = root_path.join("basedata");
        let scene_path = root_path.join("scene");
        let music_path = root_path.join("music");
        let snd_path = root_path.join("snd");

        Self {
            root_path,
            basedata_path,
            scene_path,
            music_path,
            snd_path,
        }
    }

    pub fn load_scn(&self, cpk_name: &str, scn_name: &str) -> ScnScene {
        ScnScene::new(
            &self
                .scene_path
                .join(cpk_name)
                .join(scn_name)
                .with_extension("scn"),
        )
    }

    // TODO: Return an entity
    pub fn load_mv3(&self, role_name: &str, action_name: &str) -> Mv3File {
        mv3_load_from_file(
            self.basedata_path
                .join("ROLE")
                .join(role_name)
                .join(action_name)
                .with_extension("mv3"),
        )
        .unwrap()
    }

    pub fn mv3_path(&self, role_name: &str, action_name: &str) -> PathBuf {
        self.basedata_path
            .join("ROLE")
            .join(role_name)
            .join(action_name)
            .with_extension("mv3")
    }

    pub fn load_scn_pol(&self, cpk_name: &str, scn_name: &str, pol_name: &str) -> PolFile {
        pol_load_from_file(
            self.scene_path
                .join(cpk_name)
                .join(scn_name)
                .join(pol_name)
                .with_extension("pol"),
        )
        .unwrap()
    }

    pub fn load_object_item_pol(&self, obj_name: &str) -> PolFile {
        pol_load_from_file(self.get_object_item_path(obj_name)).unwrap()
    }

    pub fn load_object_item_cvd(&self, obj_name: &str) -> CvdFile {
        cvd_load_from_file(self.get_object_item_path(obj_name)).unwrap()
    }

    pub fn load_music_data(&self, music_name: &str) -> Vec<u8> {
        let path = self.music_path.join(music_name).with_extension("mp3");
        std::fs::read(path).unwrap()
    }

    pub fn load_snd_data(&self, snd_name: &str) -> Vec<u8> {
        let path = self.snd_path.join(snd_name).with_extension("wav");
        std::fs::read(path).unwrap()
    }

    fn get_object_item_path(&self, obj_name: &str) -> PathBuf {
        if obj_name.contains('.') {
            self.basedata_path.join("object").join(&obj_name)
        } else {
            self.basedata_path
                .join("item")
                .join(&obj_name)
                .join(&obj_name)
                .with_extension("pol")
        }
    }
}
