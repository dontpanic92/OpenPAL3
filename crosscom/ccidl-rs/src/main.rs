use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let idl_path = args.next().map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: crosscom-ccidl <input.idl> [output.rs]");
        std::process::exit(2);
    });

    let generated = crosscom_ccidl::generate(&idl_path).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    if let Some(output_path) = args.next() {
        std::fs::write(output_path, generated.source).unwrap_or_else(|err| {
            eprintln!("failed to write generated Rust source: {err}");
            std::process::exit(1);
        });
    } else {
        print!("{}", generated.source);
    }
}
