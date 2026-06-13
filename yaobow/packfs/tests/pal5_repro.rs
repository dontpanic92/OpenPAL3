use std::path::Path;

use packfs::pkg::pkg_archive::PkgArchive;

fn reader() -> Box<dyn common::SeekRead> {
    let f = std::fs::File::open("F:/PAL5/script.pkg").unwrap();
    Box::new(std::io::BufReader::new(f))
}

// Heuristic "clean" length: longest prefix made of printable ascii + common
// whitespace. Good enough to correlate against a header length field.
fn clean_len(buf: &[u8]) -> usize {
    let mut n = 0;
    for &b in buf {
        let ok = b == b'\n' || b == b'\r' || b == b'\t' || (0x20..=0x7e).contains(&b);
        if ok {
            n += 1;
        } else {
            break;
        }
    }
    n
}

#[test]
fn find_length_field() {
    let mut ar = PkgArchive::load(reader(), "Y%H^uz6i").unwrap();
    let entries: Vec<_> = ar
        .entries
        .file_entries
        .iter()
        .filter(|e| e.fullpath.to_lowercase().ends_with(".lua"))
        .take(40)
        .cloned()
        .collect();

    // offset -> count of files where header u32 == clean_len
    let mut hits: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut samples = vec![];

    for e in &entries {
        let mut f = std::io::BufReader::new(std::fs::File::open("F:/PAL5/script.pkg").unwrap());
        use std::io::{Read, Seek, SeekFrom};
        f.seek(SeekFrom::Start(e.start_position as u64)).unwrap();
        let mut raw = vec![0u8; e.size as usize];
        f.read_exact(&mut raw).unwrap();
        let blob = miniz_oxide::inflate::decompress_to_vec_zlib(&raw).unwrap();
        if blob.len() < 1024 {
            continue;
        }
        let body = &blob[1024..];
        let plain = ar.open(Path::new(&e.fullpath)).map(|mut mf| {
            let mut v = vec![];
            mf.read_to_end(&mut v).unwrap();
            v
        });
        let plain = plain.unwrap();
        let cl = clean_len(&plain);
        samples.push((e.fullpath.clone(), body.len(), cl));

        for off in (0..1024 - 3).step_by(1) {
            let v = u32::from_le_bytes([blob[off], blob[off + 1], blob[off + 2], blob[off + 3]]);
            if v as usize == cl {
                *hits.entry(off).or_default() += 1;
            }
        }
    }

    println!("samples (path, body_len, clean_len, header[0..16]):");
    let mut hdr_hits: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for e in &entries {
        let mut f = std::io::BufReader::new(std::fs::File::open("F:/PAL5/script.pkg").unwrap());
        use std::io::{Read, Seek, SeekFrom};
        f.seek(SeekFrom::Start(e.start_position as u64)).unwrap();
        let mut raw = vec![0u8; e.size as usize];
        f.read_exact(&mut raw).unwrap();
        let blob = miniz_oxide::inflate::decompress_to_vec_zlib(&raw).unwrap();
        if blob.len() < 1024 {
            continue;
        }
        let mut mf = ar.open(Path::new(&e.fullpath)).unwrap();
        let mut plain = vec![];
        mf.read_to_end(&mut plain).unwrap();
        let cl = clean_len(&plain);
        let hh: Vec<String> = blob[0..16].iter().map(|b| format!("{b:02x}")).collect();
        println!("  body={} clean={} hdr={}  {}", blob.len() - 1024, cl, hh.join(" "), e.fullpath);
        // windowed: header u32 within [cl, cl+8]
        for off in 0..(1024 - 3) {
            let v =
                u32::from_le_bytes([blob[off], blob[off + 1], blob[off + 2], blob[off + 3]]) as usize;
            if v >= cl && v <= cl + 8 {
                *hdr_hits.entry(off).or_default() += 1;
            }
        }
    }
    let mut h: Vec<_> = hdr_hits.into_iter().filter(|(_, c)| *c >= 8).collect();
    h.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    println!("offsets where header u32 in [clean, clean+8]:");
    for (off, c) in h.iter().take(10) {
        println!("  off=0x{off:03x}  matched {c} files");
    }
    return;
}
