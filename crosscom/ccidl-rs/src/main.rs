use std::path::PathBuf;

fn main() {
    let mut mode = "rust";
    let mut positional: Vec<PathBuf> = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.to_string_lossy().as_ref() {
            "--protosept" => mode = "protosept",
            _ => positional.push(PathBuf::from(arg)),
        }
    }
    let mut args = positional.into_iter();

    let idl_path = args.next().unwrap_or_else(|| {
        eprintln!("usage: crosscom-ccidl [--protosept] <input.idl> [output]");
        std::process::exit(2);
    });

    let generated = match mode {
        "protosept" => crosscom_ccidl::generate_protosept(&idl_path),
        _ => crosscom_ccidl::generate(&idl_path),
    }
    .unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1);
    });

    if let Some(output_path) = args.next() {
        std::fs::write(output_path, generated.source).unwrap_or_else(|err| {
            eprintln!("failed to write generated source: {err}");
            std::process::exit(1);
        });
    } else {
        print!("{}", generated.source);
    }
}
