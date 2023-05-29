use std::{collections::HashMap, io::Read, path::Path};

use fileformats::rwbs::{anm::AnmAction, read_anm};
use mini_fs::{MiniFs, StoreExt};
use radiance::{
    components::mesh::skinned_mesh::AnimKeyFrame,
    math::{Quaternion, Vec3},
};

pub fn load_anm<P: AsRef<Path>>(vfs: &MiniFs, path: P) -> anyhow::Result<Vec<Vec<AnimKeyFrame>>> {
    let mut data = vec![];
    let _ = vfs.open(&path).unwrap().read_to_end(&mut data).unwrap();

    let anm = read_anm(&data)?;
    if anm.len() > 0 {
        Ok(load_anm_action(&anm[0]))
    } else {
        Ok(vec![])
    }
}

pub fn load_anm_action(action: &AnmAction) -> Vec<Vec<AnimKeyFrame>> {
    let mut frames = vec![];
    let mut bone_id_map = HashMap::new();
    let mut bone_id: i32 = -1;

    for i in 0..action.keyframes.len() {
        let current_bone_id = if action.keyframes[i].ts == 0.0 {
            frames.push(vec![]);
            bone_id += 1;
            bone_id
        } else {
            *bone_id_map
                .get(&(action.keyframes[i].pref_frame_off as usize))
                .unwrap_or(&0)
        };

        if action.kf_type == 1 {
            bone_id_map.insert(i * 36, current_bone_id);
        } else {
            bone_id_map.insert(i * 24, current_bone_id);
        }

        let bone_frame = &mut frames[current_bone_id as usize];
        bone_frame.push(AnimKeyFrame {
            rotation: Quaternion::new(
                action.keyframes[i].rot.x,
                action.keyframes[i].rot.y,
                action.keyframes[i].rot.z,
                action.keyframes[i].rot.w,
            ),
            position: Vec3::new(
                action.keyframes[i].pos.x,
                action.keyframes[i].pos.y,
                action.keyframes[i].pos.z,
            ),
            timestamp: action.keyframes[i].ts,
        })
    }

    frames
}
