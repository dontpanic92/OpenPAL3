use super::{read_dw_vec, read_string, read_vec, read_w_vec};
use byteorder::{LittleEndian, ReadBytesExt};
use radiance::math::Vec3;
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug)]
pub struct SceLocalVar {
    unknown: u8,
    unknown_vec: Vec<u8>,
}

#[derive(Debug)]
pub struct SceProc {
    id: u32,
    name: String,
    local_vars: Vec<SceLocalVar>,
    inst: Vec<u8>,
}

#[derive(Debug)]
pub struct SceProcHeader {
    id: u32,
    offset: u32,
    name: String,
}

#[derive(Debug)]
pub struct SceFile {
    proc_num: u16,
    proc_headers: Vec<SceProcHeader>,
    procs: HashMap<u32, SceProc>,
}

pub fn sce_load_from_file<P: AsRef<Path>>(path: P) -> SceFile {
    let mut reader = BufReader::new(fs::File::open(path).unwrap());
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
    let name = read_string(reader, 64).unwrap();

    SceProcHeader { id, offset, name }
}

fn read_sce_proc(reader: &mut dyn Read) -> SceProc {
    let id = reader.read_u32::<LittleEndian>().unwrap();
    let name_len = reader.read_u16::<LittleEndian>().unwrap();
    let name = read_string(reader, name_len as usize).unwrap();
    let local_var_num = reader.read_u16::<LittleEndian>().unwrap();

    let mut local_vars = vec![];
    for _ in 0..local_var_num {
        let u = reader.read_u8().unwrap();
        let size = reader.read_u16::<LittleEndian>().unwrap();
        let unknown_vec = read_vec(reader, size as usize).unwrap();
        local_vars.push(SceLocalVar {
            unknown: u,
            unknown_vec,
        });
    }

    let inst_size = reader.read_u32::<LittleEndian>().unwrap();
    let inst = read_vec(reader, inst_size as usize).unwrap();

    SceProc {
        id,
        name,
        local_vars,
        inst,
    }
}
