use binrw::binrw;

use crate::utils::{Pal4NodeSection, SizedString};

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
    duration: f32,
    is_instant: i32,
    unknown_i5: i32,
    data: Pal4NodeSection,
}

impl CameraData {
    pub fn get_look_at(&self) -> [f32; 3] {
        self.look_at
    }

    pub fn get_position(&self) -> [f32; 3] {
        let p = &self.data.root.as_ref().unwrap().children[0].properties;
        [
            p[0].f32().unwrap(),
            p[1].f32().unwrap(),
            p[2].f32().unwrap(),
        ]
    }

    pub fn keyframes(&self) -> Vec<[f32; 3]> {
        let Some(root) = self.data.root.as_ref() else {
            return Vec::new();
        };
        root.children
            .iter()
            .filter_map(|child| {
                let p = &child.properties;
                Some([p.get(0)?.f32()?, p.get(1)?.f32()?, p.get(2)?.f32()?])
            })
            .collect()
    }

    pub fn duration(&self) -> f32 {
        self.duration
    }

    pub fn is_instant(&self) -> bool {
        self.is_instant != 0
    }

    /// Returns the raw integer/float fields whose exact semantics are still
    /// unknown. Useful for debugging camera-data parsing (the field interpreted
    /// as `duration` is currently a best guess).
    pub fn debug_fields(&self) -> (i32, i32, f32, i32, f32, i32, i32) {
        (
            self.unknown_i1,
            self.unknown_i2,
            self.unknown_f1,
            self.unknown_i3,
            self.duration,
            self.is_instant,
            self.unknown_i5,
        )
    }
}
