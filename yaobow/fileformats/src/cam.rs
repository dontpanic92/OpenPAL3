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
    unknown_f2: f32,
    unknown_i4: i32,
    unknown_i5: i32,
    data: ScriptCameraLocalDataCache,
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
