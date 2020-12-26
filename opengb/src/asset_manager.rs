use crate::scene::{
    CvdModelEntity, PolModelEntity, RoleAnimation, RoleAnimationRepeatMode, RoleEntity, ScnScene,
};
use crate::utilities::StoreExt2;
use crate::{
    loaders::{
        cvd_loader::*,
        mv3_loader::*,
        nav_loader::{nav_load_from_file, NavFile},
        pol_loader::*,
        sce_loader::{sce_load_from_file, SceFile},
        scn_loader::scn_load_from_file,
    },
    material::LightMapMaterialDef,
};
use mini_fs::prelude::*;
use mini_fs::{LocalFs, MiniFs, CaselessFs};
use radiance::rendering::{ComponentFactory, MaterialDef};
use radiance::scene::{CoreEntity, Entity};
use radiance::{math::Vec3, rendering::SimpleMaterialDef};
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
        let local = CaselessFs::new(LocalFs::new(path.as_ref()));
        let vfs = MiniFs::new().mount("/", local);

        Self {
            factory,
            basedata_path: PathBuf::from("/basedata"),
            scene_path: PathBuf::from("/scene"),
            music_path: PathBuf::from("/music"),
            snd_path: PathBuf::from("/snd"),
            vfs,
        }
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

    pub fn load_nav(&self, cpk_name: &str, scn_name: &str) -> NavFile {
        let nav_path = self
            .scene_path
            .join(cpk_name)
            .join(scn_name)
            .join(scn_name)
            .with_extension("nav");
        nav_load_from_file(&self.vfs, nav_path)
    }

    pub fn load_role(self: &Rc<Self>, role_name: &str, default_action: &str) -> RoleEntity {
        RoleEntity::new(&self, &self.factory, role_name, default_action)
    }

    pub fn load_role_anim(&self, role_name: &str, action_name: &str) -> RoleAnimation {
        let path = self
            .basedata_path
            .join("ROLE")
            .join(role_name)
            .join(action_name)
            .with_extension("mv3");

        let mv3file = mv3_load_from_file(&self.vfs, path).unwrap();
        RoleAnimation::new(
            &self.factory,
            &mv3file,
            self.load_mv3_material(&mv3file),
            RoleAnimationRepeatMode::NoRepeat,
        )
    }

    fn load_mv3_material(&self, mv3file: &Mv3File) -> MaterialDef {
        let mut texture_path = mv3file.path.clone();
        texture_path.pop();
        texture_path.push(std::str::from_utf8(&mv3file.textures[0].names[0]).unwrap());

        SimpleMaterialDef::create(&mut self.vfs.open(texture_path).unwrap())
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
    ) -> Vec<CoreEntity<PolModelEntity>> {
        let path = self
            .scene_path
            .join(cpk_name)
            .join(scn_name)
            .join(pol_name)
            .with_extension("pol");
        let pol_file = pol_load_from_file(&self.vfs, &path).unwrap();
        self.load_pol_entities(&pol_file, path.to_str().unwrap())
    }

    // TODO: Return only one entity
    pub fn load_object_item_pol(&self, obj_name: &str) -> Vec<CoreEntity<PolModelEntity>> {
        let path = self.get_object_item_path(obj_name);
        let pol_file = pol_load_from_file(&self.vfs, &path).unwrap();
        self.load_pol_entities(&pol_file, path.to_str().unwrap())
    }

    // TODO: Return only one entity
    pub fn load_object_item_cvd(
        &self,
        obj_name: &str,
        position: &Vec3,
        rotation: f32,
    ) -> Vec<CoreEntity<CvdModelEntity>> {
        let path = self.get_object_item_path(obj_name);
        let mut entities = vec![];
        let cvd_file = cvd_load_from_file(&self.vfs, &path).unwrap();
        for (i, model) in cvd_file.models.iter().enumerate() {
            entities.append(&mut self.load_cvd_entities(
                &model,
                position,
                rotation,
                path.to_str().unwrap(),
            ));
        }

        entities
    }

    pub fn load_music_data(&self, music_name: &str) -> Vec<u8> {
        let path = self.music_path.join(music_name).with_extension("mp3");
        self.vfs.read_to_end(path).unwrap()
    }

    pub fn load_snd_data(&self, snd_name: &str) -> Vec<u8> {
        let path = self.snd_path.join(snd_name).with_extension("wav");
        self.vfs.read_to_end(path).unwrap()
    }

    fn load_pol_entities(
        &self,
        pol: &PolFile,
        model_path: &str,
    ) -> Vec<CoreEntity<PolModelEntity>> {
        let mut entities = vec![];
        for mesh in &pol.meshes {
            let material = &mesh.material_info[0];
            let entity = CoreEntity::new(
                PolModelEntity::new(
                    &self.factory,
                    &mesh.vertices,
                    &material.triangles,
                    self.load_pol_material(&material, model_path),
                    material.has_alpha
                ),
                "pol_obj",
            );

            entities.push(entity);
        }

        entities
    }

    fn load_cvd_entities(
        &self,
        model_node: &CvdModelNode,
        position: &Vec3,
        rotation: f32,
        model_path: &str,
    ) -> Vec<CoreEntity<CvdModelEntity>> {
        let mut entities = vec![];
        if let Some(model) = &model_node.model {
            for material in &model.mesh.materials {
                if material.triangles.is_none() {
                    continue;
                }

                for v in &model.mesh.frames {
                    let mut entity = CoreEntity::new(
                        CvdModelEntity::new(
                            &self.factory,
                            v,
                            material,
                            self.load_cvd_texture(material, model_path),
                        ),
                        "cvd_obj",
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

                    entities.push(entity);
                    break;
                }
            }
        }

        if let Some(children) = &model_node.children {
            for child in children {
                entities.append(&mut self.load_cvd_entities(child, position, rotation, model_path));
            }
        }

        entities
    }

    fn load_cvd_texture(&self, material: &CvdMaterial, model_path: &str) -> MaterialDef {
        let dds_name = material
            .texture_name
            .split_terminator('.')
            .next()
            .unwrap()
            .to_owned()
            + ".dds";
        let mut texture_path = PathBuf::from(model_path);
        texture_path.pop();
        texture_path.push(&dds_name);
        if !self.vfs.open(&texture_path).is_ok() {
            texture_path.pop();
            texture_path.push(&material.texture_name);
        }
        
        SimpleMaterialDef::create(&mut self.vfs.open(texture_path).unwrap())
    }

    fn load_pol_material(&self, material: &PolMaterialInfo, path: &str) -> MaterialDef {
        let texture_paths: Vec<PathBuf> = material
            .texture_names
            .iter()
            .map(|name| {
                name.split_terminator('.')
                    .next()
                    .and_then(|n| Some(n.to_owned() + ".dds"))
                    .and_then(|dds_name| {
                        let mut texture_path = PathBuf::from(path);
                        texture_path.pop();
                        texture_path.push(dds_name);
                        if !self.vfs.open(&texture_path).is_ok() {
                            texture_path.pop();
                            texture_path.push(name);
                        }

                        Some(texture_path)
                    })
                    .or(Some(PathBuf::from(name)))
                    .unwrap()
            })
            .collect();

        if texture_paths.len() == 1 {
            SimpleMaterialDef::create(&mut self.vfs.open(&texture_paths[0]).unwrap())
        } else {
            println!("pol lightmap texture path {:?}", &texture_paths);
            let mut readers: Vec<_> = texture_paths
                .iter()
                .map(|p| p.file_stem().and_then(
                    |_| Some(self.vfs.open(p).unwrap())))
                .collect();
            LightMapMaterialDef::create(&mut readers)
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
