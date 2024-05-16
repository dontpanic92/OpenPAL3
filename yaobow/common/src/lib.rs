pub mod read_ext;
pub mod store_ext;

use std::io::{Read, Seek, Write};

pub trait SeekRead: Read + Seek {}
impl<T> SeekRead for T where T: Read + Seek {}

pub trait SeekWrite: Write + Seek {}
impl<T> SeekWrite for T where T: Write + Seek {}
