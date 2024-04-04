use common::store_ext::StoreExt2;
use crosscom::ComRc;
use encoding::{types::Encoding, DecoderTrap};
use ini::Ini;
use mini_fs::prelude::*;
use mini_fs::MiniFs;
use radiance::comdef::{IAnimatedMeshComponent, IEntity, IScene};
use radiance::rendering::ComponentFactory;
use radiance::scene::CoreScene;
use radiance::utils::SeekRead;
use std::io::BufReader;
use std::path::PathBuf;
use std::{io, rc::Rc};

use super::comdef::IScnSceneComponent;
use super::loaders::nav_loader::nav_load_from_file;
use super::loaders::nav_loader::NavFile;
use super::loaders::pol::create_entity_from_pol_model;
use super::loaders::sce_loader::sce_load_from_file;
use super::loaders::sce_loader::SceFile;
use super::loaders::scn_loader::scn_load_from_file;
use super::scene::create_animated_mesh_from_mv3;
use super::scene::create_entity_from_cvd_model;
use super::scene::create_mv3_entity;
use super::scene::ScnScene;

pub struct AssetManager {
    factory: Rc<dyn ComponentFactory>,
    scene_path: PathBuf,
    music_path: PathBuf,
    movie_path: PathBuf,
    movie_end_path: PathBuf,
    movie_effect_path: PathBuf,
    snd_path: PathBuf,
    basedata_path: PathBuf,
    vfs: Rc<MiniFs>,
}

impl AssetManager {
    pub fn new(factory: Rc<dyn ComponentFactory>, vfs: Rc<MiniFs>) -> Self {
        Self {
            factory,
            basedata_path: PathBuf::from("/basedata/basedata"),
            scene_path: PathBuf::from("/scene"),
            music_path: PathBuf::from("/music/music/music"),
            movie_path: PathBuf::from("/movie/movie/movie"),
            movie_end_path: PathBuf::from("/movie/movie_end/movie"),
            movie_effect_path: PathBuf::from("/movie/movie/2deffect"),
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

    pub fn load_scn(self: &Rc<Self>, cpk_name: &str, scn_name: &str) -> ComRc<IScene> {
        let scene_base = self.scene_path.join(cpk_name).join(scn_name);
        let scene_path = scene_base.with_extension("scn");

        let scn_file = scn_load_from_file(&self.vfs, scene_path);
        let nav_file = self.load_nav(&scn_file.cpk_name, &scn_file.scn_base_name);

        let scene = CoreScene::create();
        let component = ScnScene::new(scene.clone(), &self, cpk_name, scn_name, scn_file, nav_file);
        scene.add_component(IScnSceneComponent::uuid(), ComRc::from_object(component));
        scene
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

    pub fn load_role(
        self: &Rc<Self>,
        role_name: &str,
        default_action: &str,
        name: String,
        visible: bool,
    ) -> Option<ComRc<IEntity>> {
        create_mv3_entity(self.clone(), role_name, default_action, name, visible).ok()
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
        entity: ComRc<IEntity>,
        role_name: &str,
        action_names: &[&'a str],
    ) -> Option<(&'a str, ComRc<IAnimatedMeshComponent>)> {
        for action_name in action_names {
            let anim = self.load_role_anim(entity.clone(), role_name, action_name);
            if anim.is_some() {
                return Some((action_name, anim.unwrap()));
            }
        }

        None
    }

    pub fn load_role_anim(
        &self,
        entity: ComRc<IEntity>,
        role_name: &str,
        action_name: &str,
    ) -> Option<ComRc<IAnimatedMeshComponent>> {
        let path = self
            .basedata_path
            .join("ROLE")
            .join(role_name)
            .join(action_name)
            .with_extension("mv3");

        create_animated_mesh_from_mv3(entity, &self.component_factory(), &self.vfs, &path).ok()
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
        is_night: bool,
        index: u16,
    ) -> Option<ComRc<IEntity>> {
        let folder = self.scene_path.join(cpk_name).join(scn_name);
        let mut paths = vec![];
        if is_night {
            paths.push(folder.join("1").join(pol_name).with_extension("pol"));
        }

        paths.push(folder.join(pol_name).with_extension("pol"));

        for path in &paths {
            if self.vfs.open(path).is_ok() {
                return Some(create_entity_from_pol_model(
                    &self.factory,
                    &self.vfs,
                    path,
                    format!("OBJECT_{}", index),
                    true,
                ));
            }
        }

        None
    }

    pub fn load_scn_cvd(
        &self,
        cpk_name: &str,
        scn_name: &str,
        cvd_name: &str,
        is_night: bool,
        index: u16,
    ) -> Option<ComRc<IEntity>> {
        let folder = self.scene_path.join(cpk_name).join(scn_name);
        let mut paths = vec![];
        if is_night {
            paths.push(folder.join("1").join(cvd_name).with_extension("cvd"));
        }

        paths.push(folder.join(cvd_name).with_extension("cvd"));

        for path in &paths {
            if self.vfs.open(path).is_ok() {
                return Some(create_entity_from_cvd_model(
                    self.factory.clone(),
                    &self.vfs,
                    path,
                    format!("OBJECT_{}", index),
                    true,
                ));
            }
        }

        None
    }

    pub fn load_object_item_pol(
        &self,
        obj_name: &str,
        index: u16,
        visible: bool,
    ) -> Option<ComRc<IEntity>> {
        let path = self.get_object_item_path(obj_name);
        if self.vfs.open(&path).is_ok() {
            Some(create_entity_from_pol_model(
                &self.factory,
                &self.vfs,
                &path,
                format!("OBJECT_{}", index),
                visible,
            ))
        } else {
            None
        }
    }

    pub fn load_object_item_cvd(
        &self,
        obj_name: &str,
        index: u16,
        visible: bool,
    ) -> Option<ComRc<IEntity>> {
        let path = self.get_object_item_path(obj_name);
        if self.vfs.open(&path).is_ok() {
            Some(create_entity_from_cvd_model(
                self.factory.clone(),
                &self.vfs,
                &path,
                format!("OBJECT_{}", index),
                visible,
            ))
        } else {
            None
        }
    }

    pub fn load_music_data(&self, music_name: &str) -> Vec<u8> {
        let path = self.music_path.join(music_name).with_extension("mp3");
        self.vfs.read_to_end(path).unwrap()
    }

    pub fn load_movie_data(&self, movie_name: &str) -> Box<dyn SeekRead> {
        let movie = self.movie_path.join(movie_name).with_extension("bik");
        let end_movie = self.movie_end_path.join(movie_name).with_extension("bik");
        let effect_movie = self
            .movie_effect_path
            .join(movie_name)
            .with_extension("bik");
        let file = self.vfs.open(movie).unwrap_or_else(|_| {
            self.vfs
                .open(end_movie)
                .unwrap_or_else(|_| self.vfs.open(effect_movie).unwrap())
        });

        Box::new(BufReader::new(file))
    }

    pub fn load_snd_data(&self, snd_name: &str) -> io::Result<Vec<u8>> {
        let path = self.snd_path.join(snd_name).with_extension("wav");
        self.vfs.read_to_end(path)
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
