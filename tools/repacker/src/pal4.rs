use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use image::{GenericImageView, ImageFormat, ImageOutputFormat};
use indicatif::ProgressBar;
use packfs::{cpk::CpkArchive, ypk::YpkWriter};

pub struct Pal4RepackConfig {
    pub input_folder: String,
    pub output_file: String,
    pub resize_texture: bool,
}

pub fn repack(config: Pal4RepackConfig) {
    let bar = ProgressBar::new_spinner();
    bar.enable_steady_tick(Duration::from_millis(100));

    let file = File::create(&config.output_file).unwrap();
    // let mut writer = ZipWriter::new(file);
    let mut writer = YpkWriter::new(Box::new(file)).unwrap();
    repack_recursive(
        &config,
        &bar,
        &PathBuf::from(&config.input_folder).join("gamedata"),
        &PathBuf::from(""),
        &mut writer,
    );
    writer.finish().unwrap();
    bar.finish();
}

fn repack_recursive(
    config: &Pal4RepackConfig,
    bar: &ProgressBar,
    file_path: &Path,
    dest_path: &Path,
    // writer: &mut ZipWriter<File>,
    writer: &mut YpkWriter,
) {
    if file_path.is_dir() {
        /*writer
        .add_directory(
            dest_path.to_str().unwrap(),
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Zstd),
        )
        .unwrap();*/
        for entry in file_path.read_dir().unwrap() {
            let dest_path =
                dest_path.join(entry.as_ref().unwrap().file_name().to_ascii_lowercase());
            repack_recursive(&config, bar, &entry.unwrap().path(), &dest_path, writer);
        }
    } else {
        let ext = file_path.extension().and_then(|ext| ext.to_str());
        match ext {
            Some("cpk") => {
                let dest_path = dest_path.parent().unwrap();
                repack_cpk(config, bar, file_path, dest_path, writer);
            }
            _ => {
                bar.set_message(format!("Processing {}", file_path.display()));
                let content = std::fs::read(file_path).unwrap();
                let (dest_path, content) = transform(config, dest_path, content);

                /*writer
                    .start_file(
                        dest_path.to_str().unwrap(),
                        SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Zstd),
                    )
                    .unwrap();
                writer.write_all(&content).unwrap();*/
                writer
                    .write_file(dest_path.to_str().unwrap(), &content)
                    .unwrap();
            }
        }
    }
}

fn repack_cpk(
    config: &Pal4RepackConfig,
    bar: &ProgressBar,
    file_path: &Path,
    dest_path: &Path,
    // writer: &mut ZipWriter<File>,
    writer: &mut YpkWriter,
) {
    let reader = Box::new(std::fs::File::open(file_path).unwrap());
    let mut archive = CpkArchive::load(reader).unwrap();

    let root = archive.build_directory();
    let entry_path = PathBuf::from("");
    let dest_path = dest_path.join(file_path.file_stem().unwrap().to_ascii_lowercase());
    repack_cpk_recursive(
        config,
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
    config: &Pal4RepackConfig,
    bar: &ProgressBar,
    archive_path: &Path,
    archive: &mut CpkArchive,
    entry_path: &Path,
    entry: &packfs::cpk::CpkEntry,
    dest_path: &Path,
    // writer: &mut ZipWriter<File>,
    writer: &mut YpkWriter,
) {
    for entry in entry.children() {
        let entry = entry.borrow();
        let name = entry.name();
        let entry_path = entry_path.join(name);
        let dest_path = dest_path.join(name.to_ascii_lowercase());

        if entry.is_dir() {
            /*writer
            .add_directory(
                dest_path.to_str().unwrap(),
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Zstd),
            )
            .unwrap();*/
            repack_cpk_recursive(
                config,
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

            let mut file = archive.open_str(entry_path.to_str().unwrap()).unwrap();
            let mut content = Vec::new();
            file.read_to_end(&mut content).unwrap();

            let (dest_path, content) = transform(config, &dest_path, content);
            /*writer
                .start_file(
                    dest_path.to_str().unwrap(),
                    SimpleFileOptions::default().compression_method(zip::CompressionMethod::Zstd),
                )
                .unwrap();
            writer.write_all(&content).unwrap();*/

            writer
                .write_file(dest_path.to_str().unwrap(), &content)
                .unwrap();
        }
    }
}

fn transform(config: &Pal4RepackConfig, dest_path: &Path, content: Vec<u8>) -> (PathBuf, Vec<u8>) {
    let ext = dest_path.extension().and_then(|ext| ext.to_str());
    match ext {
        Some("png") | Some("dds") => {
            if config.resize_texture {
                let (new_dest_path, format, output_format) = match ext {
                    Some("png") => (
                        dest_path.with_extension("png"),
                        ImageFormat::Png,
                        ImageOutputFormat::Png,
                    ),
                    Some("dds") => (
                        dest_path.with_extension("dds"),
                        ImageFormat::Dds,
                        ImageOutputFormat::Png,
                    ),
                    _ => unreachable!(),
                };
                let new_content = resize_texture(&content, format, output_format);
                match new_content {
                    Ok(content) => (new_dest_path, content),
                    Err(_) => {
                        println!("Failed to resize texture: {}", dest_path.display());
                        (dest_path.to_path_buf(), content)
                    }
                }
            } else {
                (dest_path.to_path_buf(), content)
            }
        }
        Some("imageset") => {
            let new_content = recalc_imageset(&content);
            (dest_path.to_path_buf(), new_content)
        }
        _ => (dest_path.to_path_buf(), content),
    }
}

fn resize_texture(
    content: &Vec<u8>,
    image_format: ImageFormat,
    output_format: ImageOutputFormat,
) -> anyhow::Result<Vec<u8>> {
    let img = image::load_from_memory_with_format(&content, image_format)?;
    let width = img.width();
    let height = img.height();
    let img = img.resize(width / 4, height / 4, image::imageops::FilterType::Lanczos3);
    let mut buf = Vec::new();
    img.write_to(&mut buf, output_format).unwrap();

    Ok(buf)
}

fn recalc_imageset(content: &Vec<u8>) -> Vec<u8> {
    let content = String::from_utf8_lossy(&content);
    let xpos = regex::Regex::new(r#"(XPos=")(\d+)(")"#).unwrap();
    let ypos = regex::Regex::new(r#"(YPos=")(\d+)(")"#).unwrap();
    let width = regex::Regex::new(r#"(Width=")(\d+)(")"#).unwrap();
    let height = regex::Regex::new(r#"(Height=")(\d+)(")"#).unwrap();

    let content = recalc_imageset_item(&content, &xpos);
    let content = recalc_imageset_item(&content, &ypos);
    let content = recalc_imageset_item(&content, &width);
    let content = recalc_imageset_item(&content, &height);

    content.into_bytes()
}

fn recalc_imageset_item(content: &str, re: &regex::Regex) -> String {
    let captures = re.captures_iter(&content).collect::<Vec<_>>();
    let mut result = content.to_string();
    for cap in captures.iter().rev() {
        let value = cap[2].parse::<u32>().unwrap();
        let new_value = value / 4;
        result.replace_range(cap.get(2).unwrap().range(), &new_value.to_string());
    }

    result
}
