use std::io::{Cursor, Read};

use byteorder::ReadBytesExt;
use common::read_ext::ReadExt;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ScriptTypeDefinition {
    name: String,
}

impl ScriptTypeDefinition {
    fn load(cursor: &mut dyn Read) -> Result<Self> {
        let name = read_string(cursor)?;

        Ok(Self { name })
    }
}

#[derive(Debug, Serialize)]
pub struct ScriptTypeReference {
    name: String,
}

impl ScriptTypeReference {
    fn load(cursor: &mut dyn Read) -> Result<Self> {
        let name = read_string(cursor)?;

        Ok(Self { name })
    }
}

#[derive(Debug, Serialize)]
pub struct UnknownAClass8 {
    flag: u8,
    unknown: u32,
    type_ref: ScriptTypeReference,
    unknown2: u8,
    unknown3: u8,
    unknown4: u8,
    unknown5: u8,
}

impl UnknownAClass8 {
    fn load(cursor: &mut dyn Read) -> Result<Self> {
        let flag = cursor.read_u8()?;
        if flag != 0 {
            unimplemented!("AClass8.flag != 0 unimplemented yet");
        }

        let unknown = cursor.read_u32_le()?;
        let type_ref = ScriptTypeReference::load(cursor)?;
        let unknown2 = cursor.read_u8()?;
        let unknown3 = cursor.read_u8()?;
        let unknown4 = cursor.read_u8()?;
        let unknown5 = cursor.read_u8()?;

        Ok(Self {
            flag,
            unknown,
            type_ref,
            unknown2,
            unknown3,
            unknown4,
            unknown5,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Instruction {
    inst: u32,
    params: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct UnknownAStruct2 {
    name: String,
    aclass8: UnknownAClass8,
    aclass8_vec: Vec<UnknownAClass8>,
    unknown_dword1: u32,
    instructions: Vec<Instruction>,
    type_refs: Vec<ScriptTypeReference>,
    dword_with_type_ref: Vec<u32>,
    unknown_dword: u32,
    type_ref: ScriptTypeReference,
    dword_vec: Vec<u32>,
}

impl UnknownAStruct2 {
    fn load(cursor: &mut dyn Read) -> Result<Self> {
        println!("Loading astruct2");

        let name = read_string(cursor)?;
        println!("{}", name);

        let aclass8 = UnknownAClass8::load(cursor)?;
        let aclass8_count = cursor.read_u32_le()? as usize;
        let mut aclass8_vec = vec![];

        for _ in 0..aclass8_count {
            aclass8_vec.push(UnknownAClass8::load(cursor)?);
        }

        let unknown_dword1 = cursor.read_u32_le()?;
        let count2 = cursor.read_u32_le()?;
        let instructions = Self::read_instructions(cursor, count2 as usize)?;
        let type_ref_count = cursor.read_u32_le()? as usize;

        let mut type_refs = vec![];
        let mut dword_with_type_ref = vec![];
        for _ in 0..type_ref_count {
            type_refs.push(ScriptTypeReference::load(cursor)?);
            dword_with_type_ref.push(cursor.read_u32_le()?);
        }

        let unknown_dword = cursor.read_u32_le()?;
        let type_ref = ScriptTypeReference::load(cursor)?;
        let dword_count = cursor.read_u32_le()? as usize;
        let mut dword_vec = vec![];
        for _ in 0..dword_count {
            dword_vec.push(cursor.read_u32_le()?);
        }

        Ok(Self {
            name,
            aclass8,
            aclass8_vec,
            unknown_dword1,
            instructions,
            type_refs,
            dword_with_type_ref,
            unknown_dword,
            type_ref,
            dword_vec,
        })
    }

    fn read_instructions(cursor: &mut dyn Read, total_size: usize) -> Result<Vec<Instruction>> {
        let mut i = 0;
        let mut instructions = vec![];

        while i < total_size {
            let inst = cursor.read_u8()?;
            let inst_len = INST_LENGTH[inst as usize];
            i += inst_len;

            let extra_len = inst_len - 4;
            let params = cursor.read_u8_vec(extra_len)?;
            instructions.push(Instruction {
                inst: inst as u32,
                params,
            });
        }

        Ok(instructions)
    }
}

#[derive(Debug, Serialize)]
pub struct ScriptModule {
    type_defs: Vec<ScriptTypeDefinition>,
    type_refs: Vec<ScriptTypeReference>,
    unknown_count3: usize,
    unknown_count4: usize,
    astruct: UnknownAStruct2,
    astruct2: UnknownAStruct2,

    astruct_vec1: Vec<UnknownAStruct2>,
    string_vec: Vec<String>,
    astruct_vec2: Vec<UnknownAStruct2>,
}

impl ScriptModule {
    pub fn load_from_buffer(buffer: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(buffer);
        Self::load(&mut cursor)
    }

    fn load(cursor: &mut Cursor<&[u8]>) -> Result<Self> {
        cursor.set_position(4);

        let type_def_count = cursor.read_u32_le()?;
        let mut type_defs = vec![];
        for _ in 0..type_def_count {
            type_defs.push(ScriptTypeDefinition::load(cursor)?);
        }

        let type_ref_count = cursor.read_u32_le()?;
        let mut type_refs = vec![];
        for _ in 0..type_ref_count {
            type_refs.push(ScriptTypeReference::load(cursor)?);
        }

        let unknown_count3 = cursor.read_u32_le()? as usize;
        let unknown_count4 = cursor.read_u32_le()? as usize;

        let astruct = UnknownAStruct2::load(cursor)?;
        let astruct2 = UnknownAStruct2::load(cursor)?;

        let astruct_count1 = cursor.read_u32_le()? as usize;
        let mut astruct_vec1 = vec![];
        for _ in 0..astruct_count1 {
            astruct_vec1.push(UnknownAStruct2::load(cursor)?);
        }

        let string_count = cursor.read_u32_le()? as usize;
        let mut string_vec = vec![];
        for _ in 0..string_count {
            string_vec.push(read_string(cursor)?);
        }

        
        let astruct_count2 = cursor.read_u32_le()? as usize;
        let mut astruct_vec2 = vec![];
        for _ in 0..astruct_count2 {
            astruct_vec2.push(UnknownAStruct2::load(cursor)?);
        }

        Ok(Self {
            type_defs,
            type_refs,
            unknown_count3,
            unknown_count4,
            astruct,
            astruct2,
            astruct_vec1,
            string_vec,
            astruct_vec2,
        })
    }
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn read_string(cursor: &mut dyn Read) -> Result<String> {
    let len = cursor.read_u32_le()?;
    cursor.read_string(len as usize)
}

const INST_LENGTH: [usize; 256] = [
    0x06, 0x06, 0x08, 0x04, 0x06, 0x04, 0x04, 0x06, 0x06, 0x04, 0x04, 0x04, 0x08, 0x06, 0x08, 0x08,
    0x08, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x06, 0x08, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x06, 0x08, 0x08, 0x08, 0x08, 0x08,
    0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x08, 0x04, 0x0C, 0x08, 0x06,
    0x06, 0x06, 0x08, 0x04, 0x04, 0x04, 0x06, 0x06, 0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
