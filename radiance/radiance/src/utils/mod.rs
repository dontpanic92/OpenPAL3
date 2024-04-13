pub mod act_drop;
pub mod free_view;
pub mod interp_value;
pub mod ray_casting;

use std::io::{Read, Seek};

pub trait SeekRead: Read + Seek {}
impl<T> SeekRead for T where T: Read + Seek {}
