use byteorder::{LittleEndian, ReadBytesExt};
use encoding::{types::Encoding, DecoderTrap};
use std::io::Read;

pub trait ReadExt: Read {
    fn read_u8_vec(&mut self, size: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut buf = vec![0u8; size];
        self.read_exact(&mut buf.as_mut_slice())?;
        Ok(buf)
    }

    fn read_dw_vec(&mut self, count: usize) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
        let mut buf = vec![0u32; count];
        self.read_u32_into::<LittleEndian>(&mut buf)?;
        Ok(buf)
    }

    fn read_w_vec(&mut self, count: usize) -> Result<Vec<u16>, Box<dyn std::error::Error>> {
        let mut buf = vec![0u16; count];
        self.read_u16_into::<LittleEndian>(&mut buf)?;
        Ok(buf)
    }

    fn read_f32_vec(&mut self, count: usize) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let mut buf = vec![0f32; count];
        self.read_f32_into::<LittleEndian>(&mut buf)?;
        Ok(buf)
    }

    fn read_string(&mut self, size: usize) -> Result<String, Box<dyn std::error::Error>> {
        let name = self.read_u8_vec(size)?;

        let name_s = encoding::all::GBK.decode(
            &name
                .into_iter()
                .take_while(|&c| c != 0)
                .collect::<Vec<u8>>(),
            DecoderTrap::Ignore,
        )?;

        Ok(name_s)
    }
}

impl<T: Read + ?Sized> ReadExt for T {}
