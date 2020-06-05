use crate::loaders::{
    cvd_loader::*,
    mv3_loader::*,
    nav_loader::{nav_load_from_file, NavFile},
    pol_loader::*,
    sce_loader::{sce_load_from_file, SceFile},
    scn_loader::scn_load_from_file,
};
use radiance::rendering::ComponentFactory;
use crate::scene::{RoleAnimation, RoleAnimationRepeatMode, RoleEntity, PolModelEntity, ScnScene, CvdModelEntity};
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub struct AssetManager {
    factory: Rc<dyn ComponentFactory>,
    root_path: PathBuf,
    scene_path: PathBuf,
    music_path: PathBuf,
    snd_path: PathBuf,
    basedata_path: PathBuf,
}

impl AssetManager {
    pub fn new<P: AsRef<Path>>(factory: Rc<dyn ComponentFactory>, path: P) -> Self {
        let root_path = path.as_ref().to_owned();
        let basedata_path = root_path.join("basedata");
        let scene_path = root_path.join("scene");
        let music_path = root_path.join("music");
        let snd_path = root_path.join("snd");

        Self {
            factory,
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

        ScnScene::new(&self, cpk_name, sn_name, scn_file, nav_file)
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
        RoleEntity::new(&self, &self.factory, role_name, default_action)
    }

    pub fn load_role_anim(&self, role_name: &str, action_name: &str) -> RoleAnimation {
        RoleAnimation::new(
            &self.factory,
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

    pub fn load_scn_pol(&self, cpk_name: &str, scn_name: &str, pol_name: &str) -> Vec<CoreEntity<PolModelEntity>> {
        let pol_file = pol_load_from_file(
            self.scene_path
                .join(cpk_name)
                .join(scn_name)
                .join(pol_name)
                .with_extension("pol"),
        )
        .unwrap();
        self.load_pol_entities(&pol_file)
    }

    pub fn load_object_item_pol(&self, obj_name: &str) -> Vec<CoreEntity<PolModelEntity>> {
        let pol_file = pol_load_from_file(self.get_object_item_path(obj_name)).unwrap();
        self.load_pol_entities(&pol_file)
    }

    pub fn load_object_item_cvd(&self, obj_name: &str, position: &Vec3, rotation: f32) -> Vec<CoreEntity<CvdModelEntity>> {
        let cvd_file = cvd_load_from_file(self.get_object_item_path(obj_name)).unwrap();
        for (i, model) in cvd_file.models.iter().enumerate() {
            cvd_add_model_entity(
                &model,
                name,
                scene,
                &model_path,
                i as u32,
                position,
                rotation,
            );
        }
    }

    pub fn load_music_data(&self, music_name: &str) -> Vec<u8> {
        let path = self.music_path.join(music_name).with_extension("mp3");
        std::fs::read(path).unwrap()
    }

    pub fn load_snd_data(&self, snd_name: &str) -> Vec<u8> {
        let path = self.snd_path.join(snd_name).with_extension("wav");
        std::fs::read(path).unwrap()
    }

    fn load_pol_entities(&self, pol: &PolFile, position: &Vec3, rotation: f32) -> Vec<CoreEntity<PolModelEntity>> {
        let mut entities = vec![];
        for mesh in &pol.meshes {
            let material = &mesh.material_info[0];
            let mut entity = CoreEntity::new(
                PolModelEntity::new(self.factory, &mesh.vertices, material, &model_path),
                &format!("{}_{}", name, i),
            );

            entities.push(entity);
        }

        entities
    }

    fn load_cvd_entities(&self, model_node: &CvdModelNode, position: &Vec3, rotation: f32) -> Vec<CoreEntity<CvdModelEntity>> {
        let entities = vec![];
        if let Some(model) = &model_node.model {
            for material in &model.mesh.materials {
                if material.triangles.is_none() {
                    continue;
                }
    
                for v in &model.mesh.frames {
                    let mut entity = CoreEntity::new(
                        CvdModelEntity::new(self.factory, v, material, path, id),
                        &format!("{}_{}", name, id),
                    );
                    let transform = entity
                        .transform_mut()
                        .set_position(position)
                        .rotate_axis_angle_local(&Vec3::UP, rotation);
    
                    if let Some(p) = model
                        .position_keyframes
                        .as_ref()
                        .and_then(|frame| frame.frames.get(0))
                        .and_then(|f| Some(&f.position))
                    {
                        transform.translate_local(p);
                    }
    
                    transform.scale_local(&Vec3::new(
                        model.scale_factor,
                        model.scale_factor,
                        model.scale_factor,
                    ));
    
                    if let Some(q) = model
                        .rotation_keyframes
                        .as_ref()
                        .and_then(|frame| frame.frames.get(0))
                        .and_then(|f| Some(&f.quaternion))
                    {
                        transform.rotate_quaternion_local(q);
                    }
    
                    if let Some(s) = model
                        .scale_keyframes
                        .as_ref()
                        .and_then(|frame| frame.frames.get(0))
                    {
                        let q2 = s.quaternion;
                        let mut q3 = q2;
                        q3.inverse();
    
                        transform
                            .rotate_quaternion_local(&q2)
                            .scale_local(&s.scale)
                            .rotate_quaternion_local(&q3);
                    }
    
                    entities.add_entity(entity);
                    break;
                }
            }
        }
    
        if let Some(children) = &model_node.children {
            for child in children {
                entities.append(self.load_cvd_entities(child));
            }
        }
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
