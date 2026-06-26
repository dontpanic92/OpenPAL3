use packfs::pkg::pkg_archive::PkgArchive;

fn main() {
    let f = std::fs::File::open("F:/Pal5/Map.pkg").unwrap();
    let ar = PkgArchive::load(Box::new(std::io::BufReader::new(f)), "Y%H^uz6i").unwrap();
    let filt = std::env::args().nth(1).unwrap_or_default().to_lowercase();
    let mut n = 0;
    for e in ar.entries.file_entries.iter() {
        let p = e.fullpath.to_lowercase();
        if p.ends_with(".ctr") && (filt.is_empty() || p.contains(&filt)) {
            println!("{}", e.fullpath);
            n += 1;
            if n >= 30 {
                break;
            }
        }
    }
    println!("(showing {} ctr entries)", n);
}
