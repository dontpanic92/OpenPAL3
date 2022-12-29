use crate::utilities::ReadExt;
use byteorder::{LittleEndian, ReadBytesExt};
use mini_fs::{MiniFs, StoreExt};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct SceLocalVar {
    pub unknown: u8,
    pub unknown_vec: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct SceProc {
    pub id: u32,
    pub name: String,
    pub local_vars: Vec<SceLocalVar>,
    pub inst: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct SceProcHeader {
    pub id: u32,
    pub offset: u32,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SceFile {
    pub proc_num: u16,
    pub proc_headers: Vec<SceProcHeader>,
    pub procs: HashMap<u32, SceProc>,
}

pub fn sce_load_from_file<P: AsRef<Path>>(vfs: &MiniFs, path: P) -> SceFile {
    let mut reader = BufReader::new(vfs.open(path).unwrap());
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).unwrap();

    match magic {
        [0x53, 0x43, 0x45, 0x00] => (), // "SCE"
        _ => panic!("Not a valid sce file"),
    }

    let magic2 = reader.read_u8().unwrap();
    if magic2 != 1 {
        panic!("Not a valid sce file");
    }

    let proc_num = reader.read_u16::<LittleEndian>().unwrap();
    let mut proc_headers = vec![];
    for _ in 0..proc_num {
        let header = read_sce_proc_header(&mut reader);
        proc_headers.push(header);
    }

    let mut procs = HashMap::new();
    for _ in 0..proc_num {
        let proc = read_sce_proc(&mut reader);
        procs.insert(proc.id, proc);
    }

    SceFile {
        proc_num,
        proc_headers,
        procs,
    }
}

fn read_sce_proc_header(reader: &mut dyn Read) -> SceProcHeader {
    let id = reader.read_u32::<LittleEndian>().unwrap();
    let offset = reader.read_u32::<LittleEndian>().unwrap();
    let name = reader.read_string(64).unwrap();

    SceProcHeader { id, offset, name }
}

fn read_sce_proc(reader: &mut dyn Read) -> SceProc {
    let id = reader.read_u32::<LittleEndian>().unwrap();
    let name_len = reader.read_u16::<LittleEndian>().unwrap();
    let name = reader.read_string(name_len as usize).unwrap();
    let local_var_num = reader.read_u16::<LittleEndian>().unwrap();

    let mut local_vars = vec![];
    for _ in 0..local_var_num {
        let u = reader.read_u8().unwrap();
        let size = reader.read_u16::<LittleEndian>().unwrap();
        let unknown_vec = reader.read_u8_vec(size as usize).unwrap();
        local_vars.push(SceLocalVar {
            unknown: u,
            unknown_vec,
        });
    }

    let inst_size = reader.read_u32::<LittleEndian>().unwrap();
    let inst = reader.read_u8_vec(inst_size as usize).unwrap();

    SceProc {
        id,
        name,
        local_vars,
        inst,
    }
}
