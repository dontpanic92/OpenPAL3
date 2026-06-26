use packfs::pkg::pkg_archive::PkgArchive;
use std::io::Read;
use std::path::Path;

fn main() {
    let inner = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"Map\kuangfengzhai\kuangfengzhai_0_0.ctr".to_string());
    let out = std::env::args()
        .nth(2)
        .unwrap_or_else(|| std::env::temp_dir().join("dump.ctr").to_string_lossy().into());
    let f = std::fs::File::open("F:/Pal5/Map.pkg").unwrap();
    let mut ar = PkgArchive::load(Box::new(std::io::BufReader::new(f)), "Y%H^uz6i").unwrap();
    let mut mf = ar.open(Path::new(&inner)).unwrap();
    let mut v = vec![];
    mf.read_to_end(&mut v).unwrap();
    std::fs::write(&out, &v).unwrap();
    println!("wrote {} bytes to {}", v.len(), out);
}
