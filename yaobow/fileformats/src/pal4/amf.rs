use binrw::binrw;

use crate::utils::SizedString;

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct AmfFile {
    header: SizedString,
    count: u32,

    #[br(count = count)]
    data: Vec<AmfEvent>,
}

impl AmfFile {
    pub fn get_event(&self, name: &str) -> Option<&AmfEvent> {
        self.data.iter().find(|d| d.get_name().as_str() == name)
    }

    pub fn events(&self) -> &Vec<AmfEvent> {
        &self.data
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct AmfEvent {
    model_name: [u8; 0x3c],
    unknown_cd: u32,
    action_name: [u8; 0x3c],
    unknown_cd2: u32,
    event_name: [u8; 0x3c],
    unknown_cd3: u32,
    tick: f32,
    unknown: u32,
    unknown_rest: [u8; 0x84],
}

impl AmfEvent {
    pub fn get_name(&self) -> String {
        let mut name = String::new();
        for c in self.event_name.iter() {
            if *c == 0 {
                break;
            }
            name.push(*c as char);
        }
        name
    }

    pub fn get_tick(&self) -> f32 {
        self.tick
    }
}
