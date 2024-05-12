use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use indicatif::ProgressBar;
use packfs::cpk::CpkArchive;
use zip::{write::SimpleFileOptions, ZipWriter};

pub struct Pal4RepackConfig {
    pub input_folder: String,
    pub output_file: String,
    pub resize_texture: bool,
}

pub fn repack(config: Pal4RepackConfig) {
    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));

    let file = File::create(&config.output_file).unwrap();
    let mut writer = ZipWriter::new(file);
    repack_recursive(
        &bar,
        &PathBuf::from(config.input_folder).join("gamedata"),
        &PathBuf::from(""),
        &mut writer,
    );
    bar.finish();
}

fn repack_recursive(
    bar: &ProgressBar,
    file_path: &Path,
    dest_path: &Path,
    writer: &mut ZipWriter<File>,
) {
    if file_path.is_dir() {
        writer
            .add_directory(
                dest_path.to_str().unwrap(),
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Zstd),
            )
            .unwrap();
        for entry in file_path.read_dir().unwrap() {
            let dest_path =
                dest_path.join(entry.as_ref().unwrap().file_name().to_ascii_lowercase());
            repack_recursive(bar, &entry.unwrap().path(), &dest_path, writer);
        }
    } else {
        let ext = file_path.extension().and_then(|ext| ext.to_str());
        match ext {
            Some("cpk") => {
                let dest_path = dest_path.parent().unwrap();
                repack_cpk(bar, file_path, dest_path, writer);
            }
            _ => {
                bar.set_message(format!("Processing {}", file_path.display()));
                let content = std::fs::read(file_path).unwrap();
                writer
                    .start_file(
                        dest_path.to_str().unwrap(),
                        SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Zstd),
                    )
                    .unwrap();
                writer.write_all(&content).unwrap();
            }
        }
    }
}

fn repack_cpk(bar: &ProgressBar, file_path: &Path, dest_path: &Path, writer: &mut ZipWriter<File>) {
    let reader = Box::new(std::fs::File::open(file_path).unwrap());
    let mut archive = CpkArchive::load(reader).unwrap();

    let root = archive.build_directory();
    let entry_path = PathBuf::from("");
    let dest_path = dest_path.join(file_path.file_stem().unwrap().to_ascii_lowercase());
    repack_cpk_recursive(
        bar,
        &file_path,
        &mut archive,
        &entry_path,
        &root,
        &dest_path,
        writer,
    );
}

fn repack_cpk_recursive(
    bar: &ProgressBar,
    archive_path: &Path,
    archive: &mut CpkArchive,
    entry_path: &Path,
    entry: &packfs::cpk::CpkEntry,
    dest_path: &Path,
    writer: &mut ZipWriter<File>,
) {
    for entry in entry.children() {
        let entry = entry.borrow();
        let name = entry.name();
        let entry_path = entry_path.join(name);
        let dest_path = dest_path.join(name.to_ascii_lowercase());

        if entry.is_dir() {
            writer
                .add_directory(
                    dest_path.to_str().unwrap(),
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Zstd),
                )
                .unwrap();
            repack_cpk_recursive(
                bar,
                archive_path,
                archive,
                &entry_path,
                &entry,
                &dest_path,
                writer,
            );
        } else {
            bar.set_message(format!(
                "Processing {}\\{}",
                archive_path.display(),
                entry_path.display(),
            ));

            let content = archive.open_str(entry_path.to_str().unwrap()).unwrap();
            writer
                .start_file(
                    dest_path.to_str().unwrap(),
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Zstd),
                )
                .unwrap();
            writer.write_all(&content.content()).unwrap();
        }
    }
}
