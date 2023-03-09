use std::io::Cursor;

use fileformats::{
    binrw::BinRead,
    nif::{
        blocks::{NiBlockArgs, NiBlocks},
        header::NiHeader,
    },
};

pub fn test_nif(data: &[u8]) {
    let mut cursor = Cursor::new(data);
    let header = NiHeader::read(&mut cursor).unwrap();
    let blocks = NiBlocks::read_args(
        &mut cursor,
        NiBlockArgs {
            block_sizes: &header.block_size,
            block_types: &header.block_types,
            block_type_index: &header.block_type_index,
        },
    );

    // println!("{}", UnknownBlock::read(&mut cursor));
    println!("{:?}", blocks);
}

pub fn run_opengujian() {
    // zpk::zpk_test().unwrap();
    // shared::fs::zpkg::zpkg_test();
    let data = std::fs::read("F:\\gujian_extracted\\101\\101.nif").unwrap();
    test_nif(&data);
}
