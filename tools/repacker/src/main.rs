mod pal4;

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 4 {
        eprintln!("Usage: {} <game> <input_folder> <output_file>", args[0]);
        std::process::exit(1);
    }

    let game = args[1].as_str();
    let input_folder = args[2].as_str();
    let output_file = args[3].as_str();
    match game {
        "--pal4" => {
            pal4::repack(pal4::Pal4RepackConfig {
                input_folder: input_folder.to_string(),
                output_file: output_file.to_string(),
                resize_texture: true,
            });

            println!("Repacking done!");
        }
        _ => {
            eprintln!("Unknown game: {}", game);
            std::process::exit(1);
        }
    }
}
