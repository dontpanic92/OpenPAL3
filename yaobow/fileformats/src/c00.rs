use binrw::BinRead;

#[derive(Debug, BinRead)]
#[brw(little)]
pub struct C00 {
    pub header: C00Header,

    #[br(count = header.packed_size as usize)]
    pub data: Vec<u8>,
}


#[derive(Debug, BinRead)]
#[brw(little)]
pub struct C00Header {
    // 32 integers
    #[br(count = 32)]
    _integers: Vec<i32>,

    #[br(calc = _integers[2])]
    pub original_size: i32,

    #[br(calc = _integers[3])]
    pub packed_size: i32,
}
