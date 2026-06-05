//! Local `Read+Seek`/`Write+Seek` trait aliases used by the asset
//! module. Kept inside `radiance` so the engine has no dependency on
//! `yaobow/common` (which carries the equivalent definitions for
//! `packfs`).

use std::io::{Read, Seek, Write};

pub trait SeekRead: Read + Seek {}
impl<T> SeekRead for T where T: Read + Seek {}

pub trait SeekWrite: Write + Seek {}
impl<T> SeekWrite for T where T: Write + Seek {}
