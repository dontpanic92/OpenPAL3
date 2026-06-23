use packfs::pkg::pkg_archive::PkgArchive;
use std::io::Read;
use std::path::Path;

fn main() {
    let f = std::fs::File::open("F:/PAL5/Map.pkg").unwrap();
    let mut ar = PkgArchive::load(Box::new(std::io::BufReader::new(f)), "Y%H^uz6i").unwrap();
    let envs: Vec<String> = ar
        .entries
        .file_entries
        .iter()
        .map(|e| e.fullpath.clone())
        .filter(|p| p.to_lowercase().ends_with("envinfo.env"))
        .collect();

    let mut total = 0;
    let mut flag_ok = 0; // body+0x44 == 1u32
    let mut cc1_ok = 0;  // body+0x54 == 0xcc (delim after color1)
    let mut cc2_ok = 0;  // body+0x58 == 0xcc (delim after color2)
    let mut fog_alpha_ok = 0; // body+0x33 == 0xff
    let mut year_ok = 0; // body+0x40 in 2000..2030
    let mut amb_ok = 0;  // ambient in 0..1.2
    let mut min_len = usize::MAX;
    let mut count_hist: std::collections::BTreeMap<u8, usize> = Default::default();

    for p in &envs {
        let mut mf = ar.open(Path::new(p)).unwrap();
        let mut v = vec![];
        mf.read_to_end(&mut v).unwrap();
        total += 1;
        min_len = min_len.min(v.len());
        let b = 12usize;
        let g = |o: usize| v.get(b + o).copied();
        let u32at = |o: usize| {
            let s = b + o;
            u32::from_le_bytes([v[s], v[s + 1], v[s + 2], v[s + 3]])
        };
        let f32at = |o: usize| {
            let s = b + o;
            f32::from_le_bytes([v[s], v[s + 1], v[s + 2], v[s + 3]])
        };
        if v.len() >= b + 0x48 && u32at(0x44) == 1 {
            flag_ok += 1;
        }
        if g(0x54) == Some(0xcc) {
            cc1_ok += 1;
        }
        if g(0x58) == Some(0xcc) {
            cc2_ok += 1;
        }
        if g(0x33) == Some(0xff) {
            fog_alpha_ok += 1;
        }
        if v.len() >= b + 0x44 {
            let y = u32at(0x40);
            if (2000..2030).contains(&y) {
                year_ok += 1;
            }
        }
        let a = f32at(0x00);
        if a >= 0.0 && a <= 1.2 {
            amb_ok += 1;
        }
        if let Some(c) = g(0x4d) {
            *count_hist.entry(c).or_default() += 1;
        }
    }
    println!("total={} min_len={}", total, min_len);
    println!("flag(body+0x44==1)   ok: {}/{}", flag_ok, total);
    println!("cc1(body+0x54==cc)   ok: {}/{}", cc1_ok, total);
    println!("cc2(body+0x58==cc)   ok: {}/{}", cc2_ok, total);
    println!("fog_alpha(0x33==ff)  ok: {}/{}", fog_alpha_ok, total);
    println!("year in 2000..2030   ok: {}/{}", year_ok, total);
    println!("ambient[0] in 0..1.2 ok: {}/{}", amb_ok, total);
    println!("light_count(body+0x4d) histogram: {:?}", count_hist);
}
