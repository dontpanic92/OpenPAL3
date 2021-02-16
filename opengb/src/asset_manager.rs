use crate::loaders::{
    mv3_loader::*,
    nav_loader::{nav_load_from_file, NavFile},
    sce_loader::{sce_load_from_file, SceFile},
    scn_loader::scn_load_from_file,
};
use crate::utilities::StoreExt2;
use crate::{
    cpk::CpkFs,
    scene::{
        CvdModelEntity, PolModelEntity, RoleAnimation, RoleAnimationRepeatMode, RoleEntity,
        ScnScene,
    },
};
use encoding::{types::Encoding, DecoderTrap};
use ini::Ini;
use log::debug;
use mini_fs::prelude::*;
use mini_fs::{LocalFs, MiniFs};
use radiance::rendering::SimpleMaterialDef;
use radiance::rendering::{ComponentFactory, MaterialDef};
use radiance::scene::CoreEntity;
use std::rc::Rc;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct AssetManager {
    factory: Rc<dyn ComponentFactory>,
    scene_path: PathBuf,
    music_path: PathBuf,
    snd_path: PathBuf,
    basedata_path: PathBuf,
    vfs: MiniFs,
}

impl AssetManager {
    pub fn new<P: AsRef<Path>>(factory: Rc<dyn ComponentFactory>, path: P) -> Self {
        let local = LocalFs::new(path.as_ref());
        let vfs = MiniFs::new(false).mount("/", local);
        let vfs = Self::mount_cpk_recursive(vfs, path.as_ref(), &PathBuf::from("./"));
        Self {
            factory,
            basedata_path: PathBuf::from("/basedata/basedata"),
            scene_path: PathBuf::from("/scene"),
            music_path: PathBuf::from("/music/music/music"),
            snd_path: PathBuf::from("/snd"),
            vfs,
        }
    }

    pub fn vfs(&self) -> &MiniFs {
        &self.vfs
    }

    pub fn component_factory(&self) -> Rc<dyn ComponentFactory> {
        self.factory.clone()
    }

    pub fn load_scn(self: &Rc<Self>, cpk_name: &str, scn_name: &str) -> ScnScene {
        let scene_base = self.scene_path.join(cpk_name).join(scn_name);
        let scene_path = scene_base.with_extension("scn");

        let scn_file = scn_load_from_file(&self.vfs, scene_path);
        let nav_file = self.load_nav(&scn_file.cpk_name, &scn_file.scn_base_name);

        ScnScene::new(&self, cpk_name, scn_name, scn_file, nav_file)
    }

    pub fn load_sce(&self, cpk_name: &str) -> SceFile {
        let scene_base = self.scene_path.join(cpk_name).join(cpk_name);
        let sce_path = scene_base.with_extension("sce");
        sce_load_from_file(&self.vfs, sce_path)
    }

    pub fn load_init_sce(&self) -> SceFile {
        let init_sce = self.basedata_path.join("init.sce");
        sce_load_from_file(&self.vfs, init_sce)
    }

    pub fn load_nav(&self, cpk_name: &str, scn_name: &str) -> NavFile {
        let nav_path = self
            .scene_path
            .join(cpk_name)
            .join(scn_name)
            .join(scn_name)
            .with_extension("nav");
        nav_load_from_file(&self.vfs, nav_path)
    }

    pub fn load_role(self: &Rc<Self>, role_name: &str, default_action: &str) -> Option<RoleEntity> {
        RoleEntity::new(self.clone(), role_name, default_action).ok()
    }

    pub fn load_role_anim_config(&self, role_name: &str) -> Ini {
        let path = self
            .basedata_path
            .join("ROLE")
            .join(role_name)
            .join(role_name)
            .with_extension("ini");

        let mv3_ini = encoding::all::GBK
            .decode(&self.vfs.read_to_end(&path).unwrap(), DecoderTrap::Ignore)
            .unwrap();
        Ini::load_from_str(&mv3_ini).unwrap()
    }

    pub fn load_role_anim_first<'a>(
        &self,
        role_name: &str,
        action_names: &[&'a str],
    ) -> Option<(&'a str, RoleAnimation)> {
        for action_name in action_names {
            let anim = self.load_role_anim(role_name, action_name);
            if anim.is_some() {
                return Some((action_name, anim.unwrap()));
            }
        }

        None
    }

    pub fn load_role_anim(&self, role_name: &str, action_name: &str) -> Option<RoleAnimation> {
        let path = self
            .basedata_path
            .join("ROLE")
            .join(role_name)
            .join(action_name)
            .with_extension("mv3");

        mv3_load_from_file(&self.vfs, &path)
            .map(|f| {
                RoleAnimation::new(
                    &self.factory,
                    &f,
                    self.load_mv3_material(&f, &path),
                    RoleAnimationRepeatMode::NoRepeat,
                )
            })
            .ok()
    }

    pub fn load_mv3_material(&self, mv3file: &Mv3File, mv3path: &Path) -> MaterialDef {
        let mut texture_path = mv3path.to_owned();
        texture_path.pop();
        texture_path.push(std::str::from_utf8(&mv3file.textures[0].names[0]).unwrap());

        SimpleMaterialDef::create(self.vfs.open(texture_path).as_mut().ok(), false)
    }

    pub fn mv3_path(&self, role_name: &str, action_name: &str) -> PathBuf {
        self.basedata_path
            .join("ROLE")
            .join(role_name)
            .join(action_name)
            .with_extension("mv3")
    }

    pub fn load_scn_pol(
        &self,
        cpk_name: &str,
        scn_name: &str,
        pol_name: &str,
        index: u16,
    ) -> Option<CoreEntity<PolModelEntity>> {
        let path = self
            .scene_path
            .join(cpk_name)
            .join(scn_name)
            .join(pol_name)
            .with_extension("pol");
        if self.vfs.open(&path).is_ok() {
            Some(CoreEntity::new(
                PolModelEntity::new(&self.factory, &self.vfs, &path),
                format!("OBJECT_{}", index),
            ))
        } else {
            None
        }
    }

    pub fn load_scn_cvd(
        &self,
        cpk_name: &str,
        scn_name: &str,
        pol_name: &str,
        index: u16,
    ) -> Option<CoreEntity<CvdModelEntity>> {
        let path = self
            .scene_path
            .join(cpk_name)
            .join(scn_name)
            .join(pol_name)
            .with_extension("cvd");
        if self.vfs.open(&path).is_ok() {
            Some(CvdModelEntity::create(
                self.factory.clone(),
                &self.vfs,
                &path,
                format!("OBJECT_{}", index),
            ))
        } else {
            None
        }
    }

    pub fn load_object_item_pol(
        &self,
        obj_name: &str,
        index: u16,
    ) -> Option<CoreEntity<PolModelEntity>> {
        let path = self.get_object_item_path(obj_name);
        if self.vfs.open(&path).is_ok() {
            Some(CoreEntity::new(
                PolModelEntity::new(&self.factory, &self.vfs, &path),
                format!("OBJECT_{}", index),
            ))
        } else {
            None
        }
    }

    pub fn load_object_item_cvd(
        &self,
        obj_name: &str,
        index: u16,
    ) -> Option<CoreEntity<CvdModelEntity>> {
        let path = self.get_object_item_path(obj_name);
        if self.vfs.open(&path).is_ok() {
            Some(CvdModelEntity::create(
                self.factory.clone(),
                &self.vfs,
                &path,
                format!("OBJECT_{}", index),
            ))
        } else {
            None
        }
    }

    pub fn load_music_data(&self, music_name: &str) -> Vec<u8> {
        let path = self.music_path.join(music_name).with_extension("mp3");
        self.vfs.read_to_end(path).unwrap()
    }

    pub fn load_snd_data(&self, snd_name: &str) -> Vec<u8> {
        let path = self.snd_path.join(snd_name).with_extension("wav");
        self.vfs.read_to_end(path).unwrap()
    }

    fn mount_cpk_recursive(mut vfs: MiniFs, asset_path: &Path, relative_path: &Path) -> MiniFs {
        let path = asset_path.join(relative_path);
        if path.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let new_path = relative_path.join(entry.file_name());
                vfs = Self::mount_cpk_recursive(vfs, asset_path, &new_path);
            }
        } else {
            if Some(true)
                == path
                    .extension()
                    .and_then(|ext| Some(ext.to_str() == Some("cpk")))
            {
                let vfs_path = PathBuf::from("/").join(relative_path.with_extension(""));

                debug!("Mounting {:?} <- {:?}", &vfs_path, &path);
                vfs = vfs.mount(vfs_path, CpkFs::new(path).unwrap())
            }
        }

        vfs
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
