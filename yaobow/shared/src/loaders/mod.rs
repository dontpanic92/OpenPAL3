use std::{
    ffi::OsStr,
    io::Read,
    path::{Path, PathBuf},
};

use common::store_ext::StoreExt2;
use mini_fs::MiniFs;

pub mod anm;
pub mod bsp;
pub mod dff;
pub mod smp;

pub trait TextureResolver: Send + Sync {
    fn resolve_texture(
        &self,
        vfs: &MiniFs,
        model_path: &Path,
        texture_name: &str,
    ) -> Option<Vec<u8>>;
}

pub struct Pal4TextureResolver;
impl TextureResolver for Pal4TextureResolver {
    fn resolve_texture(
        &self,
        vfs: &MiniFs,
        model_path: &Path,
        texture_name: &str,
    ) -> Option<Vec<u8>> {
        let tex_path = model_path
            .parent()
            .unwrap()
            .join(texture_name.to_string() + ".dds");

        let mut data = vec![];
        let _ = vfs
            .open_with_fallback(&tex_path, &["png"])
            .ok()?
            .read_to_end(&mut data)
            .ok()?;

        Some(data)
    }
}

pub struct Pal5TextureResolver;
impl TextureResolver for Pal5TextureResolver {
    fn resolve_texture(
        &self,
        vfs: &MiniFs,
        model_path: &Path,
        texture_name: &str,
    ) -> Option<Vec<u8>> {
        let tex_path = if model_path.file_name() == Some(&OsStr::new("jiemian.dff")) {
            vec![PathBuf::from(format!(
                "/Texture/load/xianjianwu/{}.dds",
                texture_name
            ))]
        } else {
            let mut paths = vec![];

            let relative_path: PathBuf = model_path
                .parent()
                .unwrap()
                .to_owned()
                .iter()
                .skip(2)
                .collect();
            let tex_path = PathBuf::from("/Texture")
                .join(relative_path)
                .join(texture_name.to_string() + ".dds");

            paths.push(tex_path.clone());

            if model_path.to_str().unwrap().contains("jianzhu") {
                let building_path = tex_path.parent().unwrap().parent().unwrap();
                let tex_path = building_path
                    .join("ZhuangShi")
                    .join(texture_name.to_string() + ".dds");
                paths.push(tex_path.clone());

                let tex_path = building_path
                    .join("jianzhu")
                    .join("ssd")
                    .join(texture_name.to_string() + ".dds");
                paths.push(tex_path);
            }

            paths
        };

        let mut data = vec![];
        let _ = vfs
            .try_open_files(&tex_path)
            .ok()?
            .read_to_end(&mut data)
            .ok()?;

        Some(data)
    }
}

pub struct Swd5TextureResolver;
impl TextureResolver for Swd5TextureResolver {
    fn resolve_texture(&self, vfs: &MiniFs, _: &Path, texture_name: &str) -> Option<Vec<u8>> {
        let tex_path = PathBuf::from("/Texture/texture").join(texture_name.to_string() + ".png");

        let mut data = vec![];
        let _ = vfs
            .open_with_fallback(&tex_path, &["dds"])
            .ok()?
            .read_to_end(&mut data)
            .ok()?;

        Some(data)
    }
}
