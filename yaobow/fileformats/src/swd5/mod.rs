use binrw::{BinRead, BinWrite};
use encoding::{DecoderTrap, Encoding};
use serde::Serialize;

pub mod atp;
pub mod fld;
pub mod map;
pub mod mapsdat;

pub type SizedBig5String = SizedBig5StringT<u16>;
pub type Sized32Big5String = SizedBig5StringT<u32>;

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
pub struct SizedBig5StringT<
    T: for<'a> BinRead<Args<'a> = ()>
        + for<'a> BinWrite<Args<'a> = ()>
        + std::fmt::Debug
        + Into<u32>
        + Copy,
> {
    pub len: T,

    #[br(count = len.into() as usize)]
    pub data: Vec<u8>,
}

impl<
        T: for<'a> BinRead<Args<'a> = ()>
            + for<'a> BinWrite<Args<'a> = ()>
            + std::fmt::Debug
            + Into<u32>
            + Copy,
    > SizedBig5StringT<T>
{
    pub fn to_string(&self) -> String {
        encoding::all::BIG5_2003
            .decode(&self.data, DecoderTrap::Ignore)
            .unwrap_or("Cannot decode text using BIG5".to_string())
    }
}

impl<
        T: for<'a> BinRead<Args<'a> = ()>
            + for<'a> BinWrite<Args<'a> = ()>
            + std::fmt::Debug
            + Into<u32>
            + Copy,
    > Serialize for SizedBig5StringT<T>
{
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = encoding::all::BIG5_2003
            .decode(&self.data, DecoderTrap::Ignore)
            .unwrap_or("Cannot decode text using BIG5".to_string());

        serializer.serialize_str(&s)
    }
}
