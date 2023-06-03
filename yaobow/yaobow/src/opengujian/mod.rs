use std::io::Cursor;

use fileformats::{binrw::BinRead, nif::NifModel};

pub fn test_nif(data: &[u8]) {
    let model = NifModel::read(&mut Cursor::new(data));

    // println!("{}", UnknownBlock::read(&mut cursor));
    println!("{:?}", model);
}

pub fn run_opengujian() {
    // zpk::zpk_test().unwrap();
    // shared::fs::zpkg::zpkg_test();
    let data = std::fs::read("F:\\gujian_extracted\\101\\101.nif").unwrap();
    test_nif(&data);
}