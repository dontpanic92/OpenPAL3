use binrw::binrw;

use crate::utils::SizedString;

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct CameraDataFile {
    count: u32,

    #[br(count = count)]
    data: Vec<CameraData>,
}

impl CameraDataFile {
    pub fn get_camera_data(&self, name: &str) -> Option<&CameraData> {
        self.data.iter().find(|d| {
            if d.name.data().last() == Some(&0) {
                &d.name.data()[..d.name.data().len() - 1] == name.as_bytes()
            } else {
                d.name.data() == name.as_bytes()
            }
        })
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct CameraData {
    name: SizedString,

    look_at: [f32; 3],
    unknown: [f32; 3],
    unknown_i1: i32,
    unknown_i2: i32,
    unknown_f1: f32,
    unknown_i3: i32,
    speed: f32,
    is_instant: i32,
    unknown_i5: i32,
    data: ScriptCameraLocalDataCache,
}

impl CameraData {
    pub fn get_look_at(&self) -> [f32; 3] {
        self.look_at
    }

    pub fn get_position(&self) -> [f32; 3] {
        let p = &self.data.root.as_ref().unwrap().children[0].properties;
        [p[0].data, p[1].data, p[2].data]
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn is_instant(&self) -> bool {
        self.is_instant != 0
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct ScriptCameraLocalDataCache {
    version1: u32,
    version2: u32,

    #[br(if(version1 == 0 || (version1 < 2 && version2 < 2)))]
    root: Option<Node>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Node {
    name: SizedString,
    property_count: u32,

    #[br(count = property_count)]
    properties: Vec<Property>,

    children_count: u32,

    #[br(count = children_count)]
    children: Vec<Box<Node>>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Property {
    ty: u32,
    name: SizedString,
    data: f32,
}
