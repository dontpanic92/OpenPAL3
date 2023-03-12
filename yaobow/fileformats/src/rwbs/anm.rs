use binrw::binread;
use serde::Serialize;

use super::{Vec3f, Vec4f};

#[binread]
#[brw(little)]
#[derive(Debug, Serialize)]
pub struct AnmAction {
    version: u32,
    kf_type: u32,

    #[bw(calc(keyframes.len() as u32))]
    kf_num: u32,
    flags: u32,
    duration: f32,

    #[br(count = kf_num)]
    keyframes: Vec<AnmKeyFrame>,
}

#[binread]
#[brw(little)]
#[br(import(kf_type: u32))]
#[derive(Debug, Serialize)]
pub struct AnmKeyFrame {
    ts: f32,

    #[br(args{half_float: kf_type == 2})]
    rot: Vec4f,

    #[br(args{half_float: kf_type == 2})]
    pos: Vec3f,

    pref_frame_off: u32,

    #[br(if(kf_type == 2))]
    kf_offset: Option<Vec3f>,

    #[br(if(kf_type == 2))]
    kf_offset_scalar: Option<Vec3f>,
}
