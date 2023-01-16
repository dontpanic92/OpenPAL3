use byteorder::{LittleEndian, ReadBytesExt};
use encoding::{types::Encoding, DecoderTrap};
use std::io::Read;

pub trait ReadExt: Read {
    fn skip(&mut self, size: usize) -> std::io::Result<()> {
        let mut buf = vec![0u8; size];
        self.read_exact(&mut buf)?;
        Ok(())
    }

    fn read_u32_le(&mut self) -> std::io::Result<u32> {
        self.read_u32::<LittleEndian>()
    }

    fn read_u16_le(&mut self) -> std::io::Result<u16> {
        self.read_u16::<LittleEndian>()
    }

    fn read_u8_vec(&mut self, size: usize) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        self.read_exact(&mut buf.as_mut_slice())?;
        Ok(buf)
    }

    fn read_dw_vec(&mut self, count: usize) -> std::io::Result<Vec<u32>> {
        let mut buf = vec![0u32; count];
        self.read_u32_into::<LittleEndian>(&mut buf)?;
        Ok(buf)
    }

    fn read_w_vec(&mut self, count: usize) -> std::io::Result<Vec<u16>> {
        let mut buf = vec![0u16; count];
        self.read_u16_into::<LittleEndian>(&mut buf)?;
        Ok(buf)
    }

    fn read_f32_vec(&mut self, count: usize) -> std::io::Result<Vec<f32>> {
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
