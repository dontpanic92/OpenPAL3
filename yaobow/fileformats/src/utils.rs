use std::borrow::Cow;

use binrw::{binrw, BinRead, BinResult, BinWrite};
use common::read_ext::FileReadError;
use encoding::{DecoderTrap, Encoding};
use serde::Serialize;

pub trait SeekRead: std::io::Read + std::io::Seek {}
impl<T> SeekRead for T where T: std::io::Read + std::io::Seek {}

pub fn to_gbk_string(v: &[u8]) -> Result<String, FileReadError> {
    let str = encoding::all::GBK
        .decode(v, DecoderTrap::Ignore)
        .map_err(|_| FileReadError::StringDecodeError)?;
    Ok(str)
}

pub fn to_big5_string(v: &[u8]) -> Result<String, FileReadError> {
    let str = encoding::all::BIG5_2003
        .decode(v, DecoderTrap::Ignore)
        .map_err(|_| FileReadError::StringDecodeError)?;
    Ok(str)
}

#[binrw]
#[brw(little)]
#[derive(Clone)]
pub struct SizedString {
    #[bw(calc(string.len() as u32))]
    size: u32,

    #[br(count = size)]
    string: Vec<u8>,
}

impl SizedString {
    pub fn data(&self) -> &[u8] {
        &self.string
    }

    pub fn to_string(&self) -> Result<String, FileReadError> {
        let slice = if self.string.last() == Some(&0) {
            &self.string[..self.string.len() - 1]
        } else {
            &self.string
        };

        to_gbk_string(slice)
    }
}

impl<T: AsRef<str>> From<T> for SizedString {
    fn from(value: T) -> Self {
        Self {
            string: value.as_ref().as_bytes().to_vec(),
        }
    }
}

impl From<SizedString> for String {
    fn from(value: SizedString) -> Self {
        String::from_utf8_lossy(&value.string).to_string()
    }
}

impl std::fmt::Debug for SizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SizedString(\"{}\")",
            String::from_utf8_lossy(&self.string)
        )
    }
}

impl PartialEq<&str> for SizedString {
    fn eq(&self, other: &&str) -> bool {
        String::from_utf8_lossy(&self.string) == *other
    }
}

impl Serialize for SizedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let str = to_gbk_string(self.data());
        serializer.serialize_str(&str.unwrap())
    }
}

#[binrw::parser(reader, endian)]
pub fn parse_sized_string() -> BinResult<String> {
    let sized_string = SizedString::read_options(reader, endian, ())?;
    let s = sized_string.to_string().unwrap_or_else(|_| {
        log::error!("Failed to decode string: {:?}", sized_string);
        "Failed to decode string".to_string()
    });

    Ok(s)
}

#[binrw]
#[brw(little)]
#[derive(Clone)]
#[brw(import(capacity: u32))]
pub struct StringWithCapacity {
    #[br(count = capacity)]
    string: Vec<u8>,
}

impl StringWithCapacity {
    pub fn data(&self) -> &[u8] {
        &self.string
    }

    pub fn as_str(&self) -> Result<String, FileReadError> {
        let end = self
            .string
            .iter()
            .position(|x| *x == 0)
            .unwrap_or(self.string.len());

        to_gbk_string(&self.string[..end])
    }
}

impl<T: AsRef<str>> From<T> for StringWithCapacity {
    fn from(value: T) -> Self {
        Self {
            string: value.as_ref().as_bytes().to_vec(),
        }
    }
}

impl From<StringWithCapacity> for String {
    fn from(value: StringWithCapacity) -> Self {
        String::from_utf8_lossy(&value.string).to_string()
    }
}

impl std::fmt::Debug for StringWithCapacity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StringWithCapacity(\"{:?}\")", self.as_str(),)
    }
}

impl PartialEq<&str> for StringWithCapacity {
    fn eq(&self, other: &&str) -> bool {
        match self.as_str() {
            Err(_) => false,
            Ok(s) => s == *other,
        }
    }
}

impl Serialize for StringWithCapacity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let str = self.as_str().unwrap_or_default();
        serializer.serialize_str(&str)
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Pal4NodeSection {
    version1: u32,
    version2: u32,

    #[br(if(version1 == 0 || (version1 < 2 && version2 < 2)))]
    pub root: Option<Pal4Node>,
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Pal4Node {
    pub name: SizedString,
    pub property_count: u32,

    #[br(count = property_count)]
    pub properties: Vec<Pal4NodeProperty>,

    pub children_count: u32,

    #[br(count = children_count)]
    pub children: Vec<Box<Pal4Node>>,
}

impl Pal4Node {
    pub fn get_child_by_name(&self, name: &str) -> Option<&Pal4Node> {
        self.children.iter().find(|c| c.name == name).map(|c| &**c)
    }

    pub fn get_property_by_name(&self, name: &str) -> Option<&Pal4NodeProperty> {
        self.properties
            .iter()
            .find(|p| *p.name() == name)
            .map(|p| p)
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub enum Pal4NodeProperty {
    #[br(magic(2u32))]
    Float(Pal4NodePropertyValue<f32>),

    #[br(magic(3u32))]
    String(Pal4NodePropertyValue<SizedString>),
}

impl Pal4NodeProperty {
    pub fn name(&self) -> &SizedString {
        match self {
            Self::Float(v) => &v.name,
            Self::String(v) => &v.name,
        }
    }

    pub fn f32(&self) -> Option<f32> {
        if let Self::Float(v) = self {
            Some(v.value)
        } else {
            None
        }
    }

    pub fn string(&self) -> Option<Cow<str>> {
        if let Self::String(v) = self {
            Some(String::from_utf8_lossy(v.value.data()))
        } else {
            None
        }
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct Pal4NodePropertyValue<
    T: for<'a> BinRead<Args<'a> = ()> + for<'a> BinWrite<Args<'a> = ()>,
> {
    pub name: SizedString,
    pub value: T,
}
