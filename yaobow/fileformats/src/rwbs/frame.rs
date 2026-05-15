use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use serde::Serialize;

use crate::rwbs::Vec3f;

use super::{extension::Extension, plugins::hanim::HAnimPlugin};

#[derive(Debug, Serialize)]
pub struct Frame {
    pub right: Vec3f,
    pub up: Vec3f,
    pub at: Vec3f,
    pub pos: Vec3f,
    pub parent: i32,
    /// Matrix flags inherited from the parent (RW per-frame flags).
    /// Bit 1 (`0x2`) indicates the frame participates in HAnim as a bone.
    /// Historically parsed as an anonymous `unknown` field.
    pub matrix_flags: u32,

    pub extensions: Vec<Extension>,
}

/// Bit in `Frame::matrix_flags` indicating the frame is an HAnim bone.
pub const FRAME_MATRIX_FLAG_HANIM_BONE: u32 = 0x2;

impl Frame {
    pub fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let right = Self::read_vec3(cursor)?;
        let up = Self::read_vec3(cursor)?;
        let at = Self::read_vec3(cursor)?;
        let pos = Self::read_vec3(cursor)?;
        let parent = cursor.read_i32::<LittleEndian>()?;
        let matrix_flags = cursor.read_u32_le()?;

        Ok(Self {
            right,
            up,
            at,
            pos,
            parent,
            matrix_flags,
            extensions: vec![],
        })
    }

    /// Raw parent matrix-flags field, as stored in the RW frame record.
    pub fn matrix_flags(&self) -> u32 {
        self.matrix_flags
    }

    /// Whether the frame is tagged as an HAnim bone by its matrix flags.
    /// Some exporters set this bit independently of attaching an
    /// `HAnimPlugin` extension, so callers that care about bone identity
    /// should cross-check with [`Frame::hanim_plugin`].
    pub fn is_hanim_bone(&self) -> bool {
        self.matrix_flags & FRAME_MATRIX_FLAG_HANIM_BONE != 0
    }

    pub fn set_extensions(&mut self, ext: Vec<Extension>) {
        self.extensions = ext;
    }

    pub fn extensions(&self) -> &[Extension] {
        &self.extensions
    }

    pub fn hanim_plugin(&self) -> Option<&HAnimPlugin> {
        self.extensions
            .iter()
            .flat_map(|e| {
                if let Extension::HAnimPlugin(hanim) = e {
                    Some(hanim)
                } else {
                    None
                }
            })
            .next()
    }

    pub fn name(&self) -> Option<String> {
        for e in &self.extensions {
            if let Extension::UserDataPlugin(u) = e {
                if let Some(names) = u.data().get("name") {
                    if let Some(name) = names.get(0).and_then(|s| s.get_string()) {
                        return Some(name);
                    }
                }
            }
        }

        for e in &self.extensions {
            if let Extension::NodeNamePlugin(n) = e {
                let trimmed = n
                    .name()
                    .trim_end_matches(|c: char| c == '\0' || c.is_whitespace());
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }

        None
    }

    fn read_vec3(cursor: &mut dyn Read) -> anyhow::Result<Vec3f> {
        let x = cursor.read_f32::<LittleEndian>()?;
        let y = cursor.read_f32::<LittleEndian>()?;
        let z = cursor.read_f32::<LittleEndian>()?;
        Ok(Vec3f { x, y, z })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rwbs::extension::{NodeNamePlugin, UserData, UserDataPlugin};
    use std::collections::HashMap;

    fn make_frame(extensions: Vec<Extension>) -> Frame {
        Frame {
            right: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
            up: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
            at: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
            pos: Vec3f { x: 0.0, y: 0.0, z: 0.0 },
            parent: -1,
            matrix_flags: 0,
            extensions,
        }
    }

    fn make_frame_with_flags(matrix_flags: u32) -> Frame {
        let mut f = make_frame(vec![]);
        f.matrix_flags = matrix_flags;
        f
    }

    #[test]
    fn matrix_flags_bit_1_marks_hanim_bone() {
        let f = make_frame_with_flags(FRAME_MATRIX_FLAG_HANIM_BONE);
        assert!(f.is_hanim_bone());
        assert_eq!(f.matrix_flags(), 0x2);
    }

    #[test]
    fn matrix_flags_zero_is_not_a_bone() {
        let f = make_frame_with_flags(0);
        assert!(!f.is_hanim_bone());
    }

    #[test]
    fn matrix_flags_other_bits_do_not_imply_bone() {
        let f = make_frame_with_flags(0xFFFF_FFFD); // every bit except bit 1
        assert!(!f.is_hanim_bone());
        assert_eq!(f.matrix_flags() & FRAME_MATRIX_FLAG_HANIM_BONE, 0);
    }

    #[test]
    fn name_falls_back_to_node_name_plugin_and_trims_padding() {
        let frame = make_frame(vec![Extension::NodeNamePlugin(
            NodeNamePlugin::new_for_test("Bip01_Head\0\0".to_string()),
        )]);
        assert_eq!(frame.name().as_deref(), Some("Bip01_Head"));
    }

    #[test]
    fn user_data_plugin_takes_priority_over_node_name_plugin() {
        let mut data = HashMap::new();
        data.insert(
            "name".to_string(),
            vec![UserData::String("FromUserData".to_string())],
        );
        let frame = make_frame(vec![
            Extension::NodeNamePlugin(NodeNamePlugin::new_for_test("FromNodeName".to_string())),
            Extension::UserDataPlugin(UserDataPlugin::new_for_test(data)),
        ]);
        assert_eq!(frame.name().as_deref(), Some("FromUserData"));
    }

    #[test]
    fn name_returns_none_when_no_name_plugin_present() {
        let frame = make_frame(vec![]);
        assert_eq!(frame.name(), None);
    }

    #[test]
    fn empty_node_name_after_trim_returns_none() {
        let frame = make_frame(vec![Extension::NodeNamePlugin(
            NodeNamePlugin::new_for_test("\0\0\0".to_string()),
        )]);
        assert_eq!(frame.name(), None);
    }
}
