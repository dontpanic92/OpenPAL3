use mini_fs::{LocalFs, MiniFs, StoreExt};
use opengb::{loaders::cvd_loader::*, material::LightMapMaterialDef};
use opengb::loaders::pol_loader::*;
use opengb::loaders::scn_loader::*;
use opengb::scene::CvdModelEntity;
use opengb::scene::PolModelEntity;
use radiance::{math::Vec3, rendering::{ComponentFactory, MaterialDef, SimpleMaterialDef}};
use radiance::scene::{CoreEntity, CoreScene, Entity, Scene, SceneExtension};
use std::{path::PathBuf, rc::Rc};

pub struct ScnScene {
    path: String,
    scn_file: ScnFile,
    vfs: MiniFs,
    factory: Rc<dyn ComponentFactory>,
}

impl SceneExtension for ScnScene {
    fn on_loading(self: &mut CoreScene<ScnScene>) {
        self
            .camera_mut()
            .transform_mut()
            .translate_local(&Vec3::new(0., 400., 1000.));

        load_scene(self, false);
    }

    fn on_updating(self: &mut CoreScene<ScnScene>, delta_sec: f32) {
        self.camera_mut().transform_mut().rotate_axis_angle(
            &Vec3::new(0., 1., 0.),
            0.2 * delta_sec * std::f32::consts::PI,
        );
    }
}

impl ScnScene {
    pub fn new(path: String, factory: Rc<dyn ComponentFactory>) -> Self {
        let local = LocalFs::new("E:\\CubeLibrary\\apps\\1000039\\basedata");
        let vfs = MiniFs::new_case_insensitive().mount("/", local);
        let scn_file = scn_load_from_file(&vfs, &path);
        println!("{:?}", scn_file);
        Self { path, scn_file, vfs, factory}
    }
}

pub fn load_scene(
    scene: &mut CoreScene<ScnScene>,
    load_objects: bool,
) {
    let scn_path = PathBuf::from(&scene.path);
    let scn_private_folder = scn_path.parent().unwrap().join(&scene.scn_file.scn_base_name);
    let object_path = scn_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("basedata")
        .join("basedata")
        .join("object");
    let item_path = scn_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("basedata")
        .join("basedata")
        .join("item");
    let pol_name = scene.scn_file.scn_base_name.clone() + ".pol";
    let pol_path = scn_private_folder.join(pol_name);
    println!("{:?}", pol_path);
    load_model(
        pol_path.to_str().unwrap(),
        "_ground",
        scene,
        &Vec3::new(0., 0., 0.),
        0.,
    );

    if !load_objects {
        return;
    }

    let mut i = 0;
    let mut nodes = vec![];
    nodes = scene.scn_file.nodes.clone();
    for obj in nodes {
        if obj.node_type != 37 && obj.node_type != 43 && obj.name.len() != 0 {
            println!("nodetype {} name {}", obj.node_type, &obj.name);
            let obj_path;
            if obj.name.as_bytes()[0] as char == '_' {
                obj_path = scn_private_folder.join(&obj.name);
            } else if obj.name.contains('.') {
                obj_path = object_path.join(&obj.name);
            } else if obj.name.as_bytes()[0] as char == '+' {
                // Unknown
                continue;
            } else {
                obj_path = item_path.join(&obj.name).join(obj.name.to_owned() + ".pol");
            }

            load_model(
                obj_path.to_str().unwrap(),
                &format!("object_{}", i),
                scene,
                &obj.position,
                obj.rotation.to_radians(),
            );

            i += 1;
        }
    }
}

fn load_model(
    model_path: &str,
    name: &str,
    scene: &mut CoreScene<ScnScene>,
    position: &Vec3,
    rotation: f32,
) {
    println!("{}", model_path);
    if model_path.to_lowercase().ends_with(".mv3") {
        /*let mut entity = CoreEntity::new(
            Mv3ModelEntity::new_from_file(&model_path, Mv3AnimRepeatMode::REPEAT),
            name,
        );
        entity
            .transform_mut()
            .set_position(position)
            .rotate_axis_angle_local(&Vec3::UP, rotation);
        scene.add_entity(entity);*/
    } else if model_path.to_lowercase().ends_with(".pol") {
        let pol = pol_load_from_file(&scene.vfs, &model_path).unwrap();
        println!("pol mesh count: {}", pol.meshes.len());
        let mut i = 0;
        for mesh in &pol.meshes {
            for material in &mesh.material_info {
                let mut entity = CoreEntity::new(
                    PolModelEntity::new(&scene.factory, &mesh.vertices, 
                        &material.triangles,
                        load_pol_material(&scene.vfs, &material, model_path),
                        material.has_alpha),
                    &format!("{}_{}", name, i),
                );
                entity
                    .transform_mut()
                    .set_position(position)
                    .rotate_axis_angle_local(&Vec3::UP, rotation);
                scene.add_entity(entity);

                i += 1;
            }
        }
    } else if model_path.to_lowercase().ends_with(".cvd") {
        /*let cvd = cvd_load_from_file(&scene.vfs, &model_path).unwrap();
        for (i, model) in cvd.models.iter().enumerate() {
            cvd_add_model_entity(
                &model,
                name,
                scene,
                &model_path,
                i as u32,
                position,
                rotation,
            );
        }*/
    } else {
        panic!("Not supported file format");
    }
}

fn load_pol_material(vfs: &MiniFs, material: &PolMaterialInfo, path: &str) -> MaterialDef {
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
                    if !vfs.open(&texture_path).is_ok() {
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
        SimpleMaterialDef::create(&mut vfs.open(&texture_paths[0]).unwrap())
    } else {
        let mut readers: Vec<_> = texture_paths
            .iter()
            .map(|p| p.file_stem().and_then(|_| Some(vfs.open(p).unwrap())))
            .collect();
        LightMapMaterialDef::create(&mut readers)
    }
}
