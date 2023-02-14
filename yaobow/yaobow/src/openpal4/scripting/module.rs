use std::{
    io::{Cursor, Read},
    rc::Rc,
};

use byteorder::ReadBytesExt;
use common::read_ext::ReadExt;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct ScriptTypeDefinition {
    name: String,
}

impl ScriptTypeDefinition {
    fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let name = read_string(cursor)?;

        Ok(Self { name })
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ScriptTypeReference {
    name: String,
}

impl ScriptTypeReference {
    fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let name = read_string(cursor)?;

        Ok(Self { name })
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ScriptDataType {
    flag: u8,
    unknown: u32,
    type_ref: ScriptTypeReference,
    unknown2: u8,
    unknown3: u8,
    unknown4: u8,
    unknown5: u8,
}

impl ScriptDataType {
    fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let flag = cursor.read_u8()?;
        if flag != 0 {
            unimplemented!("AClass8.flag != 0 unimplemented yet");
        }

        let unknown = cursor.read_u32_le()?;
        let type_ref = ScriptTypeReference::read(cursor)?;
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

#[derive(Debug, Serialize, Clone)]
pub struct Instruction {
    pub inst: u32,
    pub params: Vec<u8>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ScriptFunction {
    pub name: String,
    pub ret_type: ScriptDataType,
    pub param_types: Vec<ScriptDataType>,
    pub unknown_dword1: u32,
    pub inst: Vec<u8>,
    pub inst2: Vec<Instruction>,
    pub type_refs: Vec<ScriptTypeReference>,
    pub dword_with_type_ref: Vec<u32>,
    pub unknown_dword: u32,
    pub type_ref: ScriptTypeReference,
    pub dword_vec: Vec<u32>,
}

impl ScriptFunction {
    fn read(cursor: &mut dyn Read) -> anyhow::Result<Self> {
        let name = read_string(cursor)?;
        println!("{}", name);

        let ret_type = ScriptDataType::read(cursor)?;
        let param_count = cursor.read_u32_le()? as usize;
        let mut param_types = vec![];

        for _ in 0..param_count {
            param_types.push(ScriptDataType::read(cursor)?);
        }

        let unknown_dword1 = cursor.read_u32_le()?;
        let count2 = cursor.read_u32_le()?;
        let (inst, inst2) = Self::read_instructions(cursor, count2 as usize)?;
        let type_ref_count = cursor.read_u32_le()? as usize;

        let mut type_refs = vec![];
        let mut dword_with_type_ref = vec![];
        for _ in 0..type_ref_count {
            type_refs.push(ScriptTypeReference::read(cursor)?);
            dword_with_type_ref.push(cursor.read_u32_le()?);
        }

        let unknown_dword = cursor.read_u32_le()?;
        let type_ref = ScriptTypeReference::read(cursor)?;
        let dword_count = cursor.read_u32_le()? as usize;
        let mut dword_vec = vec![];
        for _ in 0..dword_count {
            dword_vec.push(cursor.read_u32_le()?);
        }

        Ok(Self {
            name,
            ret_type,
            param_types,
            unknown_dword1,
            inst,
            inst2,
            type_refs,
            dword_with_type_ref,
            unknown_dword,
            type_ref,
            dword_vec,
        })
    }

    fn read_instructions(
        cursor: &mut dyn Read,
        total_size: usize,
    ) -> anyhow::Result<(Vec<u8>, Vec<Instruction>)> {
        let mut i = 0;
        let mut instructions = vec![0; total_size];
        let mut instructions2 = vec![];

        while i < total_size {
            let inst = cursor.read_u8()?;
            instructions[i] = inst;
            let inst_len = INST_LENGTH[inst as usize];
            i += 4;

            let extra_len = inst_len - 4;
            cursor.read_exact(&mut instructions[i..i + extra_len])?;

            let mut p = Vec::new();
            p.extend_from_slice(&instructions[i..i + extra_len]);
            instructions2.push(Instruction {
                inst: inst as u32,
                params: p,
            });

            i += extra_len;
        }

        Ok((instructions, instructions2))
    }
}

#[derive(Debug, Serialize)]
pub struct ScriptModule {
    pub type_defs: Vec<ScriptTypeDefinition>,
    pub type_refs: Vec<ScriptTypeReference>,
    pub unknown_count3: usize,
    pub globals: Vec<u32>,
    pub module_loading: ScriptFunction,
    pub module_unloading: ScriptFunction,

    pub functions: Vec<Rc<ScriptFunction>>,
    pub strings: Vec<String>,
    pub astruct_vec2: Vec<ScriptFunction>,
}

impl ScriptModule {
    pub fn read_from_buffer(buffer: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(buffer);
        Self::read(&mut cursor)
    }

    fn read(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        cursor.set_position(4);

        let type_def_count = cursor.read_u32_le()?;
        let mut type_defs = vec![];
        for _ in 0..type_def_count {
            type_defs.push(ScriptTypeDefinition::read(cursor)?);
        }

        let type_ref_count = cursor.read_u32_le()?;
        let mut type_refs = vec![];
        for _ in 0..type_ref_count {
            type_refs.push(ScriptTypeReference::read(cursor)?);
        }

        let unknown_count3 = cursor.read_u32_le()? as usize;

        let global_count = cursor.read_u32_le()? as usize;
        let globals = vec![0; global_count];

        let module_loading = ScriptFunction::read(cursor)?;
        let module_unloading = ScriptFunction::read(cursor)?;

        let astruct_count1 = cursor.read_u32_le()? as usize;
        let mut functions = vec![];
        for _ in 0..astruct_count1 {
            functions.push(Rc::new(ScriptFunction::read(cursor)?));
        }

        let string_count = cursor.read_u32_le()? as usize;
        let mut strings = vec![];
        for _ in 0..string_count {
            strings.push(read_string(cursor)?);
        }

        let astruct_count2 = cursor.read_u32_le()? as usize;
        let mut astruct_vec2 = vec![];
        for _ in 0..astruct_count2 {
            astruct_vec2.push(ScriptFunction::read(cursor)?);
        }

        Ok(Self {
            type_defs,
            type_refs,
            unknown_count3,
            globals,
            module_loading,
            module_unloading,
            functions,
            strings,
            astruct_vec2,
        })
    }
}

fn read_string(cursor: &mut dyn Read) -> anyhow::Result<String> {
    let len = cursor.read_u32_le()?;
    cursor.read_gbk_string(len as usize)
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
