use binrw::binrw;

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct NiFooter {
    pub num_roots: u32,

    #[br(count = num_roots)]
    pub roots: Vec<i32>,
}
