use packfs::pkg::pkg_archive::PkgArchive;
use std::io::Read;
use std::path::Path;

fn main() {
    let pkg = std::env::args().nth(1).unwrap();
    let needle = std::env::args().nth(2).unwrap().to_lowercase();
    let out = std::env::args().nth(3);
    let f = std::fs::File::open(&pkg).unwrap();
    let mut ar = PkgArchive::load(Box::new(std::io::BufReader::new(f)), "Y%H^uz6i").unwrap();
    let hit: Vec<String> = ar
        .entries
        .file_entries
        .iter()
        .map(|e| e.fullpath.clone())
        .filter(|p| p.to_lowercase().contains(&needle))
        .collect();
    for (i, p) in hit.iter().enumerate().take(20) {
        println!("[{i}] {p}");
    }
    println!("({} matches)", hit.len());
    if let (Some(out), Some(first)) = (out, hit.first()) {
        let mut mf = ar.open(Path::new(first)).unwrap();
        let mut v = vec![];
        mf.read_to_end(&mut v).unwrap();
        std::fs::write(&out, &v).unwrap();
        println!("wrote {} bytes of {} to {}", v.len(), first, out);
    }
}
