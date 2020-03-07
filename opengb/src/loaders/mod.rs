pub mod mv3loader;
pub mod polloader;
pub mod cvdloader;

fn read_vec(reader: &mut dyn std::io::Read, size: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; size];
    reader.read_exact(&mut buf.as_mut_slice())?;
    Ok(buf)
}

fn calc_vertex_size(t: i32) -> usize {
    if t < 0 {
        return (t & 0x7FFFFFFF) as usize;
    }

    let mut size = 0;

    if t & 1 != 0 {
        size += 12;
    }

    if t & 2 != 0 {
        size += 12;
    }

    if t & 4 != 0 {
        size += 4;
    }
    
    if t & 8 != 0 {
        size += 4;
    }
    
    if t & 0x10 != 0 {
        size += 8;
    }
    
    if t & 0x20 != 0 {
        size += 8;
    }
    
    if t & 0x40 != 0 {
        size += 8;
    }
    
    if t & 0x80 != 0 {
        size += 8;
    }
    
    if t & 0x100 != 0 {
        size += 16;
    }

    return size;
}
