use binrw::binread;
use serde::Serialize;

use super::{Vec3f, Vec4f};

#[binread]
#[brw(little)]
#[derive(Debug, Serialize)]
pub struct AnmAction {
    version: u32,
    pub kf_type: u32,

    #[bw(calc(keyframes.len() as u32))]
    kf_num: u32,
    flags: u32,
    duration: f32,

    #[br(count = kf_num)]
    pub keyframes: Vec<AnmKeyFrame>,
}

#[binread]
#[brw(little)]
#[br(import(kf_type: u32))]
#[derive(Debug, Serialize)]
pub struct AnmKeyFrame {
    pub ts: f32,

    #[br(args{half_float: kf_type == 2})]
    pub rot: Vec4f,

    #[br(args{half_float: kf_type == 2})]
    pub pos: Vec3f,

    pub pref_frame_off: u32,

    #[br(if(kf_type == 2))]
    pub kf_offset: Option<Vec3f>,

    #[br(if(kf_type == 2))]
    pub kf_offset_scalar: Option<Vec3f>,
}
