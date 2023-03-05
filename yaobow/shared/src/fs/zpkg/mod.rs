use std::{fs::File, io::Cursor};

use byteorder::{LittleEndian, ReadBytesExt};
use common::read_ext::ReadExt;
use memmap::MmapOptions;

use crate::fs::{plain_fs::PlainArchive, zpkg::zpkg_archive::ZpkgArchive};

use self::cipher::Cipher;

pub mod cipher;
pub mod tr_cache;
pub mod zpkg_archive;
pub mod zpkg_fs;

fn select_cipher(cipher_id: u32) -> Option<Cipher> {
    unsafe {
        match cipher_id % 32 {
            0 => None,
            1 => Some(Cipher::new(56, 0x29, 2, tomcrypt_sys::blowfish_desc)),
            2 => Some(Cipher::new(16, 0x13, 2, tomcrypt_sys::cast5_desc)),
            3 => Some(Cipher::new(32, 0xc6, 4, tomcrypt_sys::rijndael_desc)),
            4 => Some(Cipher::new(16, 0xb1, 4, tomcrypt_sys::rc6_desc)),
            5 => Some(Cipher::new(96, 0x28, 2, tomcrypt_sys::rc5_desc)),
            6 => Some(Cipher::new(5, 0x13, 2, tomcrypt_sys::cast5_desc)),
            7 => Some(Cipher::new(64, 0xb1, 4, tomcrypt_sys::rc6_desc)),
            8 => Some(Cipher::new(28, 0xae, 4, tomcrypt_sys::anubis_desc)),
            9 => Some(Cipher::new(24, 0x26, 4, tomcrypt_sys::twofish_desc)),
            10 => Some(Cipher::new(32, 0x29, 2, tomcrypt_sys::blowfish_desc)),
            11 => Some(Cipher::new(24, 0xc6, 4, tomcrypt_sys::rijndael_desc)),
            12 => Some(Cipher::new(8, 0x29, 2, tomcrypt_sys::blowfish_desc)),
            13 => Some(Cipher::new(16, 0xc6, 4, tomcrypt_sys::rijndael_desc)),
            14 => Some(Cipher::new(8, 0xb1, 4, tomcrypt_sys::rc6_desc)),
            15 => Some(Cipher::new(32, 0x26, 4, tomcrypt_sys::twofish_desc)),
            16 => Some(Cipher::new(128, 0xb1, 4, tomcrypt_sys::rc6_desc)),
            17 => Some(Cipher::new(24, 0xae, 4, tomcrypt_sys::anubis_desc)),
            18 => Some(Cipher::new(16, 0xae, 4, tomcrypt_sys::anubis_desc)),
            19 => Some(Cipher::new(128, 0x28, 2, tomcrypt_sys::rc5_desc)),
            20 => Some(Cipher::new(16, 0x37, 2, tomcrypt_sys::xtea_desc)),
            21 => Some(Cipher::new(20, 0xae, 4, tomcrypt_sys::anubis_desc)),
            22 => Some(Cipher::new(8, 0xb5, 2, tomcrypt_sys::safer_k64_desc)),
            23 => Some(Cipher::new(8, 0x28, 2, tomcrypt_sys::rc5_desc)),
            24 => Some(Cipher::new(32, 0xae, 4, tomcrypt_sys::anubis_desc)),
            25 => Some(Cipher::new(16, 0x26, 4, tomcrypt_sys::twofish_desc)),
            26 => Some(Cipher::new(40, 0xae, 4, tomcrypt_sys::anubis_desc)),
            27 => Some(Cipher::new(8, 0xb5, 2, tomcrypt_sys::safer_sk64_desc)),
            28 => Some(Cipher::new(36, 0xae, 4, tomcrypt_sys::anubis_desc)),
            29 => Some(Cipher::new(16, 0xb5, 2, tomcrypt_sys::safer_sk128_desc)),
            30 => Some(Cipher::new(32, 0x28, 2, tomcrypt_sys::rc5_desc)),
            31 => Some(Cipher::new(16, 0xb5, 2, tomcrypt_sys::safer_k128_desc)),
            x => panic!("unsupported cipher {}", x),
        }
    }
}

fn decompress(buffer: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut cursor = Cursor::new(buffer);
    let total_length = cursor.read_i64::<LittleEndian>()?;
    let trunk_unpacked_size = cursor.read_i32::<LittleEndian>()? as i64;
    let trunk_num = if total_length % trunk_unpacked_size == 0 {
        total_length / trunk_unpacked_size
    } else {
        total_length / trunk_unpacked_size + 1
    } as usize;

    let mut trunk_sizes = vec![0; trunk_num];
    for i in 0..trunk_num {
        trunk_sizes[i] = cursor.read_i32::<LittleEndian>()?;
    }

    let mut output = vec![];
    for i in 0..trunk_num {
        let input = cursor.read_u8_vec(trunk_sizes[i] as usize)?;
        let mut lzma_output = vec![];
        let unpacked_size = if i == trunk_num - 1 {
            total_length - trunk_unpacked_size * (trunk_num as i64 - 1)
        } else {
            0x10000
        };

        lzma_rs::lzma_decompress_with_options(
            &mut Cursor::new(&input),
            &mut lzma_output,
            &lzma_rs::decompress::Options {
                unpacked_size: lzma_rs::decompress::UnpackedSize::UseProvided(Some(
                    unpacked_size as u64,
                )),
                memlimit: None,
                allow_incomplete: false,
            },
        )?;

        output.append(&mut lzma_output);
    }

    Ok(output)
}

fn swap_endian(data: &mut [u8]) {
    for i in 0..data.len() / 4 {
        data.swap(i * 4, i * 4 + 3);
        data.swap(i * 4 + 1, i * 4 + 2);
    }
}

pub fn zpkg_test() {
    let buffer =
        std::fs::read("F:\\SteamLibrary\\steamapps\\common\\Gujian2\\Bin\\TRGameCache.dll")
            .unwrap();

    let file = File::open("F:\\SteamLibrary\\steamapps\\common\\Gujian2\\Base.zpkg").unwrap();
    let mem = unsafe { MmapOptions::new().map(&file).unwrap() };
    let cursor = Cursor::new(mem);
    let mut archive = ZpkgArchive::load(cursor, &buffer).unwrap();

    println!("{:?}", archive);

    let _ = archive.open("Water/Caustics_22.bmp").unwrap();
}
