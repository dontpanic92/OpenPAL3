use crate::loaders::{
    cvd_loader::*,
    mv3_loader::*,
    nav_loader::{nav_load_from_file, NavFile},
    pol_loader::*,
    sce_loader::{sce_load_from_file, SceFile},
    scn_loader::scn_load_from_file,
};
use crate::scene::{RoleAnimation, RoleAnimationRepeatMode, RoleEntity, ScnScene};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct AssetManager {
    root_path: PathBuf,
    scene_path: PathBuf,
    music_path: PathBuf,
    snd_path: PathBuf,
    basedata_path: PathBuf,
}

impl AssetManager {
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

    pub fn load_scn(self: &Rc<Self>, cpk_name: &str, scn_name: &str) -> ScnScene {
        let scene_base = self.scene_path.join(cpk_name).join(scn_name);
        let scene_path = scene_base.with_extension("scn");

        let scn_file = scn_load_from_file(&scene_path);
        let nav_file = self.load_nav(&scn_file.cpk_name, &scn_file.scn_base_name);

        ScnScene::new(&self, scene_path, scn_file, nav_file)
    }

    pub fn load_sce(&self, cpk_name: &str) -> SceFile {
        let scene_base = self.scene_path.join(cpk_name).join(cpk_name);
        let sce_path = scene_base.with_extension("sce");
        sce_load_from_file(&sce_path)
    }

    pub fn load_nav(&self, cpk_name: &str, scn_name: &str) -> NavFile {
        nav_load_from_file(
            &self
                .scene_path
                .join(cpk_name)
                .join(scn_name)
                .join(scn_name)
                .with_extension("nav"),
        )
    }

    pub fn load_role(self: &Rc<Self>, role_name: &str, default_action: &str) -> RoleEntity {
        RoleEntity::new(&self, role_name, default_action)
    }

    pub fn load_role_anim(&self, role_name: &str, action_name: &str) -> RoleAnimation {
        RoleAnimation::new(
            &mv3_load_from_file(
                self.basedata_path
                    .join("ROLE")
                    .join(role_name)
                    .join(action_name)
                    .with_extension("mv3"),
            )
            .unwrap(),
            RoleAnimationRepeatMode::NoRepeat,
        )
    }

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
