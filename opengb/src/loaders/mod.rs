pub mod cvdloader;
pub mod mv3loader;
pub mod polloader;
pub mod scnloader;

use byteorder::{LittleEndian, ReadBytesExt};
use encoding::{DecoderTrap, Encoding};

fn read_vec(
    reader: &mut dyn std::io::Read,
    size: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buf = vec![0u8; size];
    reader.read_exact(&mut buf.as_mut_slice())?;
    Ok(buf)
}

fn read_dw_vec(
    reader: &mut dyn std::io::Read,
    count: usize,
) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    let mut buf = vec![0u32; count];
    reader.read_u32_into::<LittleEndian>(&mut buf)?;
    Ok(buf)
}

fn read_f32_vec(
    reader: &mut dyn std::io::Read,
    count: usize,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut buf = vec![0f32; count];
    reader.read_f32_into::<LittleEndian>(&mut buf)?;
    Ok(buf)
}

fn read_w_vec(
    reader: &mut dyn std::io::Read,
    count: usize,
) -> Result<Vec<u16>, Box<dyn std::error::Error>> {
    let mut buf = vec![0u16; count];
    reader.read_u16_into::<LittleEndian>(&mut buf)?;
    Ok(buf)
}

fn read_string(
    reader: &mut dyn std::io::Read,
    size: usize,
) -> Result<String, Box<dyn std::error::Error>> {
    let name = read_vec(reader, size).unwrap();
    println!("name {:?}", name);

    let name_s = encoding::all::GBK
        .decode(
            &name
                .into_iter()
                .take_while(|&c| c != 0)
                .collect::<Vec<u8>>(),
            DecoderTrap::Ignore,
        )
        .unwrap();

    Ok(name_s)
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
